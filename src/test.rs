use crate::*;
use KbctKeyStatus::*;
use std::collections::{HashMap, BTreeSet};

fn key(str: &str) -> i32 {
	match str {
		"A" => 1,
		"B" => 2,
		"C" => 3,
		"D" => 4,
		"1" => 11,
		"2" => 12,
		"3" => 13,
		"4" => 14,
		_ => -1
	}
}

fn create_keymap_func(f: fn(&str) -> i32) -> impl Fn(&String) -> Option<i32> {
	move |x: &String| match f(&x[..]) {
		-1 => None,
		x => Some(x)
	}
}

fn map_string(mp: HashMap<&str, &str>) -> HashMap<String, String> {
	mp.iter()
		.map(|(k, v)| (k.to_string(), v.to_string()))
		.collect()
}

fn vec_string(mp: Vec<&str>) -> Vec<String> {
	mp.iter().map(|x| x.to_string()).collect()
}

fn create_test_kbct() -> Result<Kbct> {
	Kbct::new(KbctConf {
		simple: Some(map_string(hashmap!["3" => "2"])),
		complex: Some(vec![
			KbctComplexConf {
				modifiers: vec_string(vec!["A", "B"]),
				keymap: map_string(hashmap!["1" => "2", "2" => "1"]),
			},
			KbctComplexConf {
				modifiers: vec_string(vec!["A", "C"]),
				keymap: map_string(hashmap!["2" => "3"]),
			},
			KbctComplexConf {
				modifiers: vec_string(vec!["A"]),
				keymap: map_string(hashmap!["1" => "3"]),
			}
		]),
	}, create_keymap_func(key))
}

struct KbctTestContext<'a> {
	kbct: &'a mut Kbct
}

impl<'a> KbctTestContext<'a> {
	fn run_test(&mut self, s: &str, ev_type: KbctKeyStatus, expected: Vec<(&str, KbctKeyStatus)>) {
		let exp: Vec<KbctEvent> = expected.iter().map(|(x, y)| Kbct::make_ev(key(x), *y)).collect();
		let result = self.kbct.map_event(Kbct::make_ev(key(s), ev_type));
		assert_eq!(exp, result);
	}

	fn assert_click(&mut self, key: &str, expected: Vec<(&str, KbctKeyStatus)>) {
		self.run_test(key, Clicked, expected);
	}

	fn assert_pressed(&mut self, key: &str, expected: Vec<(&str, KbctKeyStatus)>) {
		self.run_test(key, Pressed, expected);
	}

	fn assert_release(&mut self, key: &str, expected: Vec<(&str, KbctKeyStatus)>) {
		self.run_test(key, Released, expected);
	}
}


#[test]
fn test_map_event() -> Result<()> {
	let mut test = KbctTestContext { kbct: &mut create_test_kbct().unwrap() };

	// Test single key with click and press
	test.assert_click("A", vec![("A", Clicked)]);
	test.assert_pressed("A", vec![("A", Pressed)]);
	test.assert_pressed("A", vec![("A", Pressed)]);
	test.assert_release("A", vec![("A", Released)]);

	// Test single key with click
	test.assert_click("B", vec![("B", Clicked)]);
	test.assert_release("B", vec![("B", Released)]);

	// Test key combo 1
	test.assert_click("B", vec![("B", Clicked)]);
	test.assert_click("A", vec![("A", Clicked)]);
	test.assert_pressed("A", vec![("A", Pressed)]);
	test.assert_release("B", vec![("B", Released)]);
	test.assert_release("A", vec![("A", Released)]);

	// Test simple mapping
	test.assert_click("A", vec![("A", Clicked)]);
	test.assert_release("A", vec![("A", Released)]);
	test.assert_click("3", vec![("2", Clicked)]);
	test.assert_pressed("3", vec![("2", Pressed)]);
	test.assert_release("3", vec![("2", Released)]);

	// Test complex mapping
	test.assert_click("A", vec![("A", Clicked)]);
	test.assert_click("1", vec![("A", ForceReleased), ("3", Clicked)]);
	test.assert_release("1", vec![("3", Released)]);
	test.assert_click("1", vec![("3", Clicked)]);
	test.assert_click("3", vec![("A", Clicked), ("2", Clicked)]);
	// test.assert_click("4", vec![("4", Clicked)]);
	// test.assert_release("A", vec![]);
	// test.assert_release("1", vec![("3", Released)]);

	Ok(())
}

#[test]
fn test_active_mapping() -> Result<()> {
	let mut kbct = create_test_kbct()?;
	kbct.map_event(Kbct::make_ev(key("A"), Clicked));
	kbct.map_event(Kbct::make_ev(key("B"), Clicked));
	kbct.map_event(Kbct::make_ev(key("C"), Clicked));
	let active = kbct.get_active_complex_modifiers().unwrap();
	assert_eq!(btreeset![key("A"), key("C")], *active.0);


	let mut kbct = create_test_kbct()?;
	kbct.map_event(Kbct::make_ev(key("A"), Clicked));
	let active = kbct.get_active_complex_modifiers().unwrap();
	assert_eq!(btreeset![key("A")], *active.0);

	let mut kbct = create_test_kbct()?;
	kbct.map_event(Kbct::make_ev(key("B"), Clicked));
	let active = kbct.get_active_complex_modifiers();
	assert!(active.is_none());
	Ok(())
}

#[test]
fn test_create_kbct_fail() -> Result<()> {
	let kbct = Kbct::new(KbctConf {
		simple: Some(hashmap!["C".to_string() => "D".to_string()]),
		complex: Some(vec![
			KbctComplexConf {
				modifiers: vec!["A".to_string(), "B".to_string()],
				keymap: hashmap!["1".to_string() => "2".to_string(), "2".to_string() => "1".to_string()],
			},
			KbctComplexConf {
				modifiers: vec!["A".to_string()],
				keymap: hashmap!["1".to_string() => "3".to_string()],
			}
		]),
	}, |_| None);
	let err = "Configuration contains unknown keys: \
     {\"1\", \"2\", \"3\", \"A\", \"B\", \"C\", \"D\"}";
	match kbct {
		Ok(_) => assert!(false), // Has to fail
		Err(KbctError::Error(k)) => assert_eq!(err, k),
		_ => {}
	}
	Ok(())
}


#[test]
fn test_create_complex_kbct() -> Result<()> {
	let kbct = create_test_kbct()?;

	assert_eq!(1, kbct.simple_map.len());
	assert_eq!(3, kbct.complex_map.len());
	assert_eq!(hashmap![12 => 11, 11=>12], *kbct.complex_map.get(&btreeset! {1, 2}).unwrap());
	assert_eq!(hashmap![11 => 13], *kbct.complex_map.get(&btreeset! {1}).unwrap());

	Ok(())
}

#[test]
fn test_create_simple_kbct() -> Result<()> {
	let simple = hashmap! {
        "K1".to_string() => "K2".to_string()
    };
	let kbct = Kbct::new(KbctConf {
		simple: Some(simple),
		complex: None,
	}, create_keymap_func(|x| match x {
		"K1" => 1,
		"K2" => 2,
		_ => -1
	}))?;
	assert_eq!(1, kbct.simple_map.len());
	assert_eq!(2, *kbct.simple_map.get(&1).unwrap());
	Ok(())
}

#[test]
fn test_conf_parser() -> Result<()> {
	let yml = "simple:";
	let conf = KbctConf::parse(yml.to_string())?;
	assert_eq!(None, conf.simple);
	assert_eq!(None, conf.complex);

	let yml = "ignored_key: 12";
	let conf = KbctConf::parse(yml.to_string())?;
	assert_eq!(None, conf.simple);
	assert_eq!(None, conf.complex);

	let yml = "simple:\n  KEY: VALUE\n";
	let conf = KbctConf::parse(yml.to_string())?;
	assert!(conf.simple.is_some());
	assert_eq!(None, conf.complex);
	let map = conf.simple.unwrap();
	assert_eq!("VALUE", map.get("KEY").unwrap());
	assert_eq!(1, map.len());

	let yml = "complex:\n- modifiers: ['LEFT_ALT']\n  keymap:\n    KEY_I: UP_ARROW";
	let conf = KbctConf::parse(yml.to_string())?;
	assert!(conf.complex.is_some());
	let complex_vec = conf.complex.unwrap();
	let conf = complex_vec.get(0).unwrap();
	assert_eq!("UP_ARROW", conf.keymap.get("KEY_I").unwrap());
	assert_eq!(1, conf.keymap.len());
	assert_eq!(1, conf.modifiers.len());
	assert_eq!("LEFT_ALT", conf.modifiers.first().unwrap());

	let yml = "complex:\n  modifiers: ['LEFT_ALT']";
	let conf = KbctConf::parse(yml.to_string());
	assert!(conf.is_err());

	let yml = "complex:\n  keymap:\n    KEY_I: UP_ARROW";
	let conf = KbctConf::parse(yml.to_string());
	assert!(conf.is_err());

	Ok(())
}
