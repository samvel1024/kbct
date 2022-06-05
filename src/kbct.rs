#[macro_use]
extern crate maplit;

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};
use std::slice::Iter;
use std::vec::Vec;

use linked_hash_map::LinkedHashMap;
use log::{error, warn};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
struct KbctComplexConf {
	modifiers: Vec<String>,
	keymap: HashMap<String, String>,
}

pub type KbctRootConf = Vec<KbctConf>;
pub type Result<T> = std::result::Result<T, KbctError>;

type Keycode = i32;
// Mapping from one keycode to another
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

	#[error("Utf8 Error")]
	Utf8Error(#[from] std::str::Utf8Error),

	#[error("Regex Error")]
	RegexError(#[from] regex::Error),

	#[error("Kbct Error")]
	Error(String),
}

fn unwrap_kv<E>(kv: (E, E)) -> Vec<E> {
	let (k, v) = kv;
	vec![k, v]
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct KbctConf {
	keyboards: Vec<String>,
	keymap: Option<HashMap<String, String>>,
	layers: Option<Vec<KbctComplexConf>>,
}

impl KbctConf {
	pub fn keyboards(&self) -> Iter<'_, String> {
		return self.keyboards.iter();
	}
}

impl KbctConf {
	pub fn parse(str: String) -> Result<KbctConf> {
		Ok(serde_yaml::from_str(&str)?)
	}
}

#[derive(Debug)]
struct KbctKeyState {
	time: u64,
	mapped_code: Keycode,
	status: KbctKeyStatus,
}

/// Structure representing the whole user configuration
#[derive(Debug, Default)]
pub struct Kbct {
    /// Mapping `keycode -> keycode` for the default mapping
	simple_map: KeyMap,
    /// Definitions of user layers
    /// A layer is also a mapping `keycode -> keycode`.
	/// Each layer is indexed by a set of keycodes enabling it.
	complex_map: ComplexKeyMap,
    /// State of each keycode, indexed by the keycode itself
	source_to_mapped: KeyStateMap,
    /// ???
	mapped_to_source: ReverseKeyMap,
    /// Time of the internal system
	logic_clock: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
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
	pub fn new_test(simple_keymap: KeyMap, complex_keymap: ComplexKeyMap) -> Kbct {
		Kbct {
			simple_map: simple_keymap,
			complex_map: complex_keymap,
			..Default::default()
		}
	}

	pub fn new(conf: KbctConf, key_code: impl Fn(&String) -> Option<i32>) -> Result<Kbct> {
		Self::check_keys(&conf, &key_code)?;

		let str_to_code = |k| key_code(k).unwrap();
		let str_to_code_pair = |(k, v)| (str_to_code(k), str_to_code(v));
		let simple = conf.keymap.unwrap_or_default();
		let simple_map: KeyMap = simple.iter().map(str_to_code_pair).collect();

		let complex = conf.layers.unwrap_or_default();
		let complex_map: HashMap<KeySet, KeyMap> = complex
			.iter()
			.map(|x| {
				(
					x.modifiers.iter().map(str_to_code).collect(),
					x.keymap.iter().map(str_to_code_pair).collect(),
				)
			})
			.collect();

		Ok(Kbct {
			simple_map,
			complex_map,
			source_to_mapped: LinkedHashMap::new(),
			mapped_to_source: hashmap!(),
			logic_clock: 0,
		})
	}

    /// Check that the keys defined in the configuration are all valid
	fn check_keys(conf: &KbctConf, key_code: impl Fn(&String) -> Option<i32>) -> Result<()> {
		let keys = Self::collect_used_keys(conf);
		let unknown_keys: BTreeSet<&String> = keys.into_iter().filter(|x| key_code(*x).is_none()).collect();
		if !unknown_keys.is_empty() {
			Err(KbctError::Error(format!(
				"Configuration contains unknown keys: {:?}",
				unknown_keys
			)))
		} else {
			Ok(())
		}
	}

    /// Collects all keys used through the configuration
	fn collect_used_keys<'a>(conf: &'a KbctConf) -> Vec<&'a String> {
        let mut keys = Vec::new();
		if let Some(simple) = conf.keymap.as_ref() {
            let simple_keys = simple.iter().flat_map(unwrap_kv);
            keys.extend(simple_keys);
		}
		if let Some(complex) = conf.layers.as_ref() {
            let complex_keys = complex.iter().flat_map(|x| {
                x.modifiers
                    .iter()
                    .chain(x.keymap.iter().flat_map(unwrap_kv))
            });
            keys.extend(complex_keys);
		}
        keys
	}

	fn get_active_complex_modifiers(&self) -> Option<(&KeySet, &KeyMap)> {
		let cm = &self.complex_map;
		let stm = &self.source_to_mapped;

		let get_last_pressed_time = |s: &KeySet| -> u64 {
			s.iter()
				.map(|x| self.get_last_source_mapping_to(*x).unwrap())
				.map(|x| stm.get(&x).unwrap().time)
				.max()
				.unwrap()
		};

		let latest_keystroke = |l: &&KeySet, r: &&KeySet| -> Ordering {
			if l.len() == r.len() {
				get_last_pressed_time(l).cmp(&get_last_pressed_time(r))
			} else {
				l.len().cmp(&r.len())
			}
		};

		let all_pressed = |x: &&KeySet| x.iter().find(|x| stm.get(x).is_none()).is_none();

		cm.iter()
			.map(|(k, _v)| k)
			.filter(all_pressed)
			.max_by(latest_keystroke)
			.map(|x| (x, cm.get(x).unwrap()))
	}

	fn make_ev(code: Keycode, ev_type: KbctKeyStatus) -> KbctEvent {
		KbctEvent { code, ev_type }
	}

	fn change_key_state(&mut self, source: Keycode, mapped: Keycode, status: KbctKeyStatus) {
		let empty_hashet = LinkedHashSet::new();

		if status != KbctKeyStatus::Released {
			self.mapped_to_source
				.entry(mapped)
				.or_insert(empty_hashet)
				.insert(source, true);
			self.source_to_mapped.insert(
				source,
				KbctKeyState {
					time: self.logic_clock,
					mapped_code: mapped,
					status,
				},
			);
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

	fn get_last_source_mapping_to(&self, code: Keycode) -> Option<Keycode> {
		match self.mapped_to_source.get(&code) {
			Some(x) => x.iter().map(|x| *x.0).last(),
			None => None,
		}
	}

	pub fn map_event(&mut self, ev: KbctEvent) -> Vec<KbctEvent> {
		use KbctKeyStatus::*;
		let empty_map = hashmap!();
		let empty_set = btreeset!();

		let not_mapped = ev.code;
		let simple_mapped = *self.simple_map.get(&not_mapped).unwrap_or(&not_mapped);
		let (active_modifiers, complex_keymap) = self
			.get_active_complex_modifiers()
			.unwrap_or((&empty_set, &empty_map));

		let complex_mapped = *complex_keymap.get(&not_mapped).unwrap_or(&simple_mapped);

		let prev_state = self.source_to_mapped.get(&not_mapped);
		let prev_status = prev_state.map(|x| x.status).unwrap_or(Released);
		let mut result = vec![];

		match (prev_status, ev.ev_type) {
			(Released, Clicked) => {
				let synthetic_modifier_events: Vec<_> = active_modifiers
					.iter()
					.flat_map(|modifier_raw| {
						let modifier_mapped = self.source_to_mapped.get(&modifier_raw).unwrap();

						let is_complex = complex_mapped != simple_mapped;

						match (modifier_mapped.status, is_complex) {
							(Clicked, true) => {
								Some((*modifier_raw, modifier_mapped.mapped_code, ForceReleased))
							}
							(ForceReleased, false) => {
								Some((*modifier_raw, modifier_mapped.mapped_code, Clicked))
							}
							(Released, _) => panic!("Illegal state"),
							_ => None,
						}
					})
					.collect();

				for (source, mapped, status) in synthetic_modifier_events.iter() {
					self.change_key_state(*source, *mapped, *status)
				}
				self.change_key_state(not_mapped, complex_mapped, Clicked);

				result = synthetic_modifier_events
					.iter()
					.map(|(_s, target, st)| Kbct::make_ev(*target, *st))
					.collect();
				result.push(Kbct::make_ev(complex_mapped, Clicked));
			}
			(Clicked, Released) | (Pressed, Released) => {
				if prev_state.is_none() {
					warn!("WARNING: key press was not recorded, skipping");
				} else {
					let prev_mapped_code = prev_state.unwrap().mapped_code;
					let down_keys = self
						.mapped_to_source
						.get(&prev_mapped_code)
						.map(|x| x.len())
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
				warn!(
					"Illegal state transition {:?} {:?}",
					prev_status, ev.ev_type
				);
			}
		}
		result
	}
}

#[cfg(test)]
mod test;
