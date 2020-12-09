#![allow(unused_imports)]
#![allow(dead_code)]
#[macro_use]
extern crate maplit;

use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet, BTreeSet};
use std::error::Error;
use std::io;
use std::cmp::Ordering;
use std::cmp::Ordering::Less;
use thiserror::Error;
use linked_hash_map::LinkedHashMap;
use std::ptr::hash;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct KbctComplexConf {
	modifiers: Vec<String>,
	keymap: HashMap<String, String>,
}

pub type Result<T> = std::result::Result<T, KbctError>;

type Keycode = i32;
type KeyMap = HashMap<Keycode, Keycode>;
type KeySet = BTreeSet<Keycode>;
type ComplexKeyMap = HashMap<KeySet, KeyMap>;
type KeyStateMap = LinkedHashMap<Keycode, KbctKeyState>;
type LinkedHashSet<T> = LinkedHashMap<T, bool>;
type KeySequenceSet = LinkedHashSet<Keycode>;
type ReverseKeyMap = HashMap<Keycode, KeySequenceSet>;

#[derive(Error, Debug)]
pub enum KbctError {
	#[error("Uinput error `{0}`")]
	UinputError(#[from] uinput::Error),

	#[error("Json error")]
	JsonError(#[from] serde_json::Error),

	#[error("Yaml error")]
	YamlError(#[from] serde_yaml::Error),

	#[error("IO Error {0}`")]
	IOError(#[from] std::io::Error),

	#[error("Kbct Error")]
	Error(String),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct KbctRootConf {
	simple: Option<HashMap<String, String>>,
	complex: Option<Vec<KbctComplexConf>>,
}

impl KbctRootConf {
	pub fn parse(str: String) -> Result<KbctRootConf> {
		let yml = serde_yaml::from_str(&str)?;
		Ok(yml)
	}
}


#[derive(Debug)]
struct KbctKeyState {
	time: u64,
	mapped_code: Keycode,
	status: KbctKeyStatus,
}

#[derive(Debug)]
pub struct Kbct {
	simple_map: KeyMap,
	complex_map: ComplexKeyMap,
	source_to_mapped: KeyStateMap,
	mapped_to_source: ReverseKeyMap,
	logic_clock: u64,
}


#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct KbctEvent {
	pub code: Keycode,
	pub ev_type: KbctKeyStatus,
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum KbctKeyStatus {
	ForceReleased,
	Released,
	Clicked,
	Pressed,
}


impl Kbct {
	pub fn new(conf: KbctRootConf, key_code: impl Fn(&String) -> Option<i32>) -> Result<Kbct> {
		let simple = conf.simple.unwrap_or_default();
		let complex = conf.complex.unwrap_or_default();

		let unwrap_kv = |(k, v)| vec![k, v];
		let str_to_code = |k| key_code(k).unwrap();
		let str_to_code_pair = |(k, v)| (str_to_code(k), str_to_code(v));

		let all_keys = simple.iter()
			.flat_map(unwrap_kv)
			.chain(complex.iter()
				.flat_map(|x| x.modifiers.iter().chain(
					x.keymap.iter().flat_map(unwrap_kv)
				)));

		let unknown_keys: BTreeSet<&String> = all_keys.filter(|x| key_code(*x).is_none()).collect();
		if !unknown_keys.is_empty() {
			return Err(KbctError::Error(
				format!("Configuration contains unknown keys: {:?}", unknown_keys)));
		}

		let simple_map: KeyMap = simple.iter().map(str_to_code_pair).collect();

		let complex_map: HashMap<KeySet, KeyMap> = complex.iter()
			.map(|x| (
				x.modifiers.iter().map(str_to_code).collect(),
				x.keymap.iter().map(str_to_code_pair).collect()
			))
			.collect();

		Ok(Kbct {
			simple_map,
			complex_map,
			source_to_mapped: LinkedHashMap::new(),
			mapped_to_source: hashmap!(),
			logic_clock: 0,
		})
	}


	fn get_active_complex_modifiers(&self) -> Option<(&KeySet, &KeyMap)> {
		let cm = &self.complex_map;
		let mts = &self.mapped_to_source;
		let stm = &self.source_to_mapped;

		let get_last_pressed_time = |s: &KeySet| -> u64 {
			s.iter()
				.map(|x| self.get_last_source_mapping_to(*x).unwrap())
				.map(|x| stm.get(&x).unwrap().time)
				.max().unwrap()
		};

		let latest_keystroke = |l: &&KeySet, r: &&KeySet| -> Ordering {
			if l.len() == r.len() {
				get_last_pressed_time(l).cmp(&get_last_pressed_time(r))
			} else {
				l.len().cmp(&r.len())
			}
		};

		let all_pressed = |x: &&KeySet| x.iter()
			.find(|x| mts.get(x).is_none()).is_none();

		cm.iter()
			.map(|(k, _v)| k)
			.filter(all_pressed)
			.max_by(latest_keystroke)
			.map(|x| (x, cm.get(x).unwrap()))
	}

	fn map_key(k: Keycode, map: &KeyMap) -> Keycode {
		return *map.get(&k).unwrap_or(&k);
	}

	fn make_ev(code: Keycode, ev_type: KbctKeyStatus) -> KbctEvent {
		KbctEvent { code, ev_type }
	}


	fn change_key_state(&mut self, source: Keycode, mapped: Keycode, status: KbctKeyStatus) {
		let empty_hashet = LinkedHashSet::new();

		if status != KbctKeyStatus::Released {
			self.mapped_to_source.entry(mapped).or_insert(empty_hashet).insert(source, true);
			self.source_to_mapped.insert(source, KbctKeyState {
				time: self.logic_clock,
				mapped_code: mapped,
				status,
			});
		} else {
			let set = self.mapped_to_source.entry(mapped).or_insert(empty_hashet);
			set.remove(&source);
			if set.is_empty() {
				self.mapped_to_source.remove(&mapped);
			}

			self.source_to_mapped.remove(&source);
		}
		self.logic_clock += 1;
	}

	fn mark_key_clicked(&mut self, code: Keycode) {
		self.change_key_state(code, code, KbctKeyStatus::Clicked);
	}

	fn get_current_complex_mapping(&self, set: &KeySet) -> Option<&KeyMap> {
		self.complex_map.get(set)
	}

	fn get_last_source_mapping_to(&self, code: Keycode) -> Option<Keycode> {
		match self.mapped_to_source.get(&code) {
			Some(x) => x.iter().map(|x| *x.0).last(),
			None => None
		}
	}

	pub fn map_event(&mut self, ev: KbctEvent) -> Vec<KbctEvent> {
		use KbctKeyStatus::*;
		let empty_map = hashmap!();
		let empty_set = btreeset!();

		let not_mapped = ev.code;
		let simple_mapped = Kbct::map_key(ev.code, &self.simple_map);

		let (active_modifiers, complex_keymap) = self.get_active_complex_modifiers()
			.unwrap_or((&empty_set, &empty_map));
		let complex_mapped = *complex_keymap.get(&simple_mapped).unwrap_or(&simple_mapped);

		let prev_state = self.source_to_mapped.get(&not_mapped);
		let prev_status = prev_state.map(|x| x.status).unwrap_or(Released);
		let mut result = vec![];

		match (prev_status, ev.ev_type) {
			(Released, Clicked) => {
				result = active_modifiers
					.iter()
					.map(|x| (*x, self.get_last_source_mapping_to(*x).unwrap()))
					.map(|(target, source)| (target, self.source_to_mapped.get(&source).unwrap().status))
					.flat_map(|(target, status)| {
						let is_complex = complex_mapped != simple_mapped;
						match (status, is_complex) {
							(Pressed, _) => None,
							(Clicked, true) => Some((target, ForceReleased)),
							(Clicked, false) => None,
							(ForceReleased, true) => None,
							(ForceReleased, false) => Some((target, Clicked)),
							(Released, _) => panic!("")
						}
					})
					.map(|(target, status)| Kbct::make_ev(target, status))
					.collect();
				self.change_key_state(not_mapped, complex_mapped, Clicked);
				for x in &result {
					self.change_key_state(self.get_last_source_mapping_to(x.code).unwrap(), x.code, ForceReleased);
				}
				result.push(Kbct::make_ev(complex_mapped, Clicked));
			}
			(Clicked, Released) | (Pressed, Released) => {
				if prev_state.is_none() {
					println!("WARNING: key press was not recorded, skipping");
				} else {
					let prev_mapped_code = prev_state.unwrap().mapped_code;
					let down_keys = self.mapped_to_source.get(&prev_mapped_code)
						.map(|x| {
							x.len()
						})
						.unwrap_or(0);
					if down_keys == 1 {
						result.push(Kbct::make_ev(prev_mapped_code, Released));
					}
					self.change_key_state(not_mapped, prev_mapped_code, Released);
				}
			}
			(ForceReleased, Released) => {
				let prev_code = prev_state.unwrap().mapped_code;
				self.change_key_state(not_mapped, prev_code, Released);
			}
			(Clicked, Pressed) | (Pressed, Pressed) => {
				let mapped = prev_state.unwrap().mapped_code;
				result.push(Kbct::make_ev(mapped, Pressed));
			}
			(ForceReleased, Pressed) => {}
			_ => {
				panic!("Illegal state transition {:?} {:?}", prev_status, ev.ev_type);
			}
		}
		result
	}
}


#[cfg(test)]
mod tests;
