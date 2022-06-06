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

struct KeyMapping {
	source: Keycode,
	target: Keycode,
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
	/// Internal status representing a simulated release of the key
	ForceReleased,
	/// Key release
	Released,
	/// Key press
	Clicked,
	/// Key repetition (from source hardware)
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
		let unknown_keys: BTreeSet<&String> = keys
			.into_iter()
			.filter(|x| key_code(*x).is_none())
			.collect();
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

	/// Gets the definition of the active "layer".
	///
	/// ### Return
	///
	/// A tuple `(keys, mapping)` where:
	///  - `keys` contains the list of combinations activating the "layer"
	///  - `mapping` is the special mapping for this layer
	fn get_active_complex_modifiers(&self) -> Option<&KeySet> {
		let cm = &self.complex_map;
		let active_layers = cm
			.iter()
			.map(|(k, _v)| k)
			.filter(|keys| keys.iter().all(|k| self.is_pressed(*k)));
		active_layers.max_by(|a, b| self.get_latest_keystroke(a, b))
	}

	fn get_last_pressed_time(&self, s: &KeySet) -> u64 {
		let stm = &self.source_to_mapped;
		s.iter()
			.filter_map(|k| self.get_last_source_mapping_to(*k))
			.filter_map(|k| stm.get(&k))
			.map(|state| state.time)
			.max()
			.unwrap()
	}

	fn get_latest_keystroke(&self, l: &KeySet, r: &KeySet) -> Ordering {
		if l.len() == r.len() {
			self.get_last_pressed_time(l)
				.cmp(&self.get_last_pressed_time(r))
		} else {
			l.len().cmp(&r.len())
		}
	}

	/// Tests if a given physical key is pressed.
	fn is_pressed(&self, key: Keycode) -> bool {
		self.source_to_mapped.get(&key).is_some()
	}

	fn get_simple_target(&self, ev: &KbctEvent) -> Option<Keycode> {
		self.simple_map.get(&ev.code).map(|k| *k)
	}

	fn get_target_in_complex(&self, ev: &KbctEvent, active_modifiers: &KeySet) -> Option<Keycode> {
		self.complex_map
			.get(active_modifiers)
			.and_then(|layer| layer.get(&ev.code))
			.map(|k| *k)
	}

	fn build_modifier_events(
		&self,
		active_modifiers: &KeySet,
		is_complex: bool,
	) -> Vec<(KeyMapping, KbctKeyStatus)> {
		use KbctKeyStatus::*;
		active_modifiers
			.iter()
			// for all keys activating the layer...
			.map(|modifier| {
				self.source_to_mapped
					.get(modifier)
					.map(|state| (modifier, state))
					.expect(&format!(
						"Modifier {} of keyset {:?} should have been pressed to activate layer",
						modifier, active_modifiers
					))
			})
			// ... that are pressed ...
			.flat_map(|(modifier, state)| match (state.status, is_complex) {
				(Clicked, true) => Some((
					KeyMapping {
						source: *modifier,
						target: state.mapped_code,
					},
					ForceReleased,
				)),
				(ForceReleased, false) => Some((
					KeyMapping {
						source: *modifier,
						target: state.mapped_code,
					},
					Clicked,
				)),
				(Released, _) => panic!("Illegal state"),
				_ => None,
			})
			// ... build an event releasing modifier or restore it as clicked
			.collect()
	}

	fn record_order_effects(&mut self, orders: &Vec<(KeyMapping, KbctKeyStatus)>) {
		for (KeyMapping { source, target }, status) in orders.iter() {
			self.change_key_state(*source, *target, *status)
		}
	}

	fn make_events(orders: &Vec<(KeyMapping, KbctKeyStatus)>) -> Vec<KbctEvent> {
		orders
			.iter()
			.map(|(mapping, st)| Kbct::make_ev(mapping.target, *st))
			.collect()
	}

	fn make_ev(code: Keycode, ev_type: KbctKeyStatus) -> KbctEvent {
		KbctEvent { code, ev_type }
	}

	/// Updates the state of a given key.
	///
	/// ### Arguments
	///
	///  - `source` - the physical key
	///  - `mapped` - the logical key
	///  - `status` - the new state of the key
	fn change_key_state(&mut self, source: Keycode, mapped: Keycode, status: KbctKeyStatus) {
		use KbctKeyStatus::*;

		match status {
			// FIXME why recording a released as a down
			ForceReleased | Pressed | Clicked => {
				let mapping = KeyMapping {
					source,
					target: mapped,
				};
				self.record_pressed(&mapping, status);
				self.add_key_source(&mapping);
			}
			Released => {
				let mapping = KeyMapping {
					source,
					target: mapped,
				};
				self.record_release(&mapping);
				self.remove_key_source(&mapping);
			}
		}
		self.logic_clock += 1;
	}

	/// Gets the last physical keycode emitting a given logical keycode.
	///
	/// ### Arguments
	///
	///  - `target` - logical keycode to consider
	///  
	fn get_last_source_mapping_to(&self, target: Keycode) -> Option<Keycode> {
		match self.mapped_to_source.get(&target) {
			Some(x) => x.iter().map(|x| *x.0).last(),
			None => None,
		}
	}

	pub fn map_event(&mut self, ev: KbctEvent) -> Vec<KbctEvent> {
		use KbctKeyStatus::*;
		let not_mapped = ev.code;

		let prev_state = self.source_to_mapped.get(&not_mapped);
		let prev_status = prev_state.map(|x| x.status).unwrap_or(Released);
		let events = match (prev_status, ev.ev_type) {
			(Released, Clicked) => Some(self.on_click(&ev)),
			(Clicked, Released) | (Pressed, Released) => {
				// FIXME? cannot happen: prev_state = None => prev_status = Released
				if prev_state.is_none() {
					warn!("WARNING: key press was not recorded, skipping");
					None
				} else {
					Some(self.on_release(&ev))
				}
			}
			(ForceReleased, Released) => {
				self.record_forced_released(&ev);
				None
			}
			(Clicked, Pressed) | (Pressed, Pressed) => {
				Some(Self::repeat_press(&prev_state.unwrap()))
			}
			(ForceReleased, Pressed) => None,
			_ => {
				warn!(
					"Illegal state transition {:?} {:?}",
					prev_status, ev.ev_type
				);
				None
			}
		};
		events.unwrap_or_else(|| vec![])
	}

	fn on_click(&mut self, ev: &KbctEvent) -> Vec<KbctEvent> {
		use KbctKeyStatus::*;

		let simple_mapped = self.get_simple_target(ev);

		let active_modifiers = self.get_active_complex_modifiers();
		let complex_mapped = active_modifiers.and_then(|keys| self.get_target_in_complex(ev, keys));

		let mut event_orders: Vec<_> = match active_modifiers {
			Some(keys) => {
				let is_complex = complex_mapped.is_some();
				self.build_modifier_events(keys, is_complex)
			}
			None => vec![],
		};
		event_orders.push((
			KeyMapping {
				source: ev.code,
				target: complex_mapped.or(simple_mapped).unwrap_or(ev.code),
			},
			Clicked,
		));

		self.record_order_effects(&event_orders);
		Self::make_events(&event_orders)
	}

	/// Produce events on the release of keyboard keys or mouse buttons
	///
	/// ### Arguments:
	///
	///  - `ev` - the event received upon the release
	///
	/// ### Returns
	///
	/// the list of simulated events
	fn on_release(&mut self, ev: &KbctEvent) -> Vec<KbctEvent> {
		use KbctKeyStatus::*;
		let released_keycode = ev.code;
		let prev_state = self.source_to_mapped.get(&released_keycode);
		let prev_mapped_code = prev_state.unwrap().mapped_code;
		let down_keys = self
			.mapped_to_source
			.get(&prev_mapped_code)
			.map(|x| x.len());
		let mut result = vec![];
		if down_keys == Some(1) {
			result.push(Kbct::make_ev(prev_mapped_code, Released));
		}
		self.change_key_state(released_keycode, prev_mapped_code, Released);
		result
	}

	fn record_forced_released(&mut self, ev: &KbctEvent) {
		let prev_state = self.source_to_mapped.get(&ev.code);
		let prev_code = prev_state.unwrap().mapped_code;
		self.change_key_state(ev.code, prev_code, KbctKeyStatus::Released);
	}

	fn repeat_press(state: &KbctKeyState) -> Vec<KbctEvent> {
		let mapped = state.mapped_code;
		vec![Self::make_ev(mapped, KbctKeyStatus::Pressed)]
	}

	/// Records that a keycode was engaged
	///
	/// This sets in `source_to_mapped` the chosen logical target keycode for the physical keycode.
	fn record_pressed(&mut self, mapping: &KeyMapping, status: KbctKeyStatus) {
		self.source_to_mapped.insert(
			mapping.source,
			KbctKeyState {
				time: self.logic_clock,
				mapped_code: mapping.target,
				status,
			},
		);
	}

	/// Records that a keycode was released
	///
	/// This unsets the mapping from `source_to_mapped`.
	fn record_release(&mut self, mapping: &KeyMapping) {
		self.source_to_mapped.remove(&mapping.source);
	}

	/// Adds that a logical keycode has been emitted for a given physical keycode.
	///
	/// Why???
	fn add_key_source(&mut self, mapping: &KeyMapping) {
		self.mapped_to_source
			.entry(mapping.target)
			.or_insert(LinkedHashMap::new())
			.insert(mapping.source, true);
	}

	/// Removes the source of a given logical keycode.
	///
	/// Why???
	fn remove_key_source(&mut self, mapping: &KeyMapping) {
		if let Some(set) = self.mapped_to_source.get_mut(&mapping.target) {
			set.remove(&mapping.source);
			if set.is_empty() {
				self.mapped_to_source.remove(&mapping.target);
			}
		} else {
			warn!(
				"Release of {} (for source {}) without previous pressed/click",
				mapping.target, mapping.source
			);
		}
	}
}

#[cfg(test)]
mod test;
