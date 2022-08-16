use std::collections::HashMap;

use KbctKeyStatus::*;

use crate::*;

fn key(str: &str) -> i32 {
	str.as_bytes()[0] as i32
}

fn create_keymap_func(f: fn(&str) -> i32) -> impl Fn(&String) -> Option<i32> {
	move |x: &String| match f(&x[..]) {
		-1 => None,
		x => Some(x),
	}
}

fn map_string(mp: HashMap<&str, &str>) -> HashMap<String, KeyPressConf> {
	mp.iter()
		.map(|(k, v)| (k.to_string(), KeyPressConf::Key(v.to_string())))
		.collect()
}

fn vec_string(mp: Vec<&str>) -> Vec<String> {
	mp.iter().map(|x| x.to_string()).collect()
}

fn create_test_kbct() -> Result<Kbct> {
	Kbct::new(
		KbctConf {
			keyboards: vec![],
			keymap: Some(map_string(hashmap!["3" => "2"])),
			layers: Some(vec![
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
				},
			]),
		},
		create_keymap_func(key),
	)
}

struct KbctTestContext {
	kbct: Kbct,
}

impl KbctTestContext {
	fn new(
		simple: HashMap<&str, &str>,
		complex: HashMap<BTreeSet<&str>, HashMap<&str, &str>>,
	) -> KbctTestContext {
		fn name_to_codes(map: HashMap<&str, &str>) -> HashMap<i32, KeyPress> {
			map.into_iter()
				.map(|(l, r)| {
					(
						key(l),
						KeyPress {
							code: key(r),
							modifiers: Default::default(),
						},
					)
				})
				.collect()
		}

		let simple_codes = name_to_codes(simple);

		let complex_codes = complex
			.into_iter()
			.map(|(l, r)| (l.into_iter().map(|x| key(x)).collect(), name_to_codes(r)))
			.collect();

		let kbct = Kbct::new_test(simple_codes, complex_codes);
		KbctTestContext { kbct }
	}

	fn run_test(&mut self, s: &str, ev_type: KbctKeyStatus, expected: Vec<(&str, KbctKeyStatus)>) {
		let exp: Vec<KbctEvent> = expected
			.iter()
			.map(|(x, y)| Kbct::make_ev(key(x), *y))
			.collect();
		let result = self.kbct.map_event(Kbct::make_ev(key(s), ev_type));
		assert_eq!(exp, result);
	}

	fn click(&mut self, key: &str, expected: Vec<(&str, KbctKeyStatus)>) {
		self.run_test(key, Clicked, expected);
	}

	fn press(&mut self, key: &str, expected: Vec<(&str, KbctKeyStatus)>) {
		self.run_test(key, Pressed, expected);
	}

	fn release(&mut self, key: &str, expected: Vec<(&str, KbctKeyStatus)>) {
		self.run_test(key, Released, expected);
	}
}

#[test]
fn test_map_event() -> Result<()> {
	let mut test = KbctTestContext {
		kbct: create_test_kbct().unwrap(),
	};

	// Test single key with click and press
	test.click("A", vec![("A", Clicked)]);
	test.press("A", vec![("A", Pressed)]);
	test.press("A", vec![("A", Pressed)]);
	test.release("A", vec![("A", Released)]);

	// Test single key with click
	test.click("B", vec![("B", Clicked)]);
	test.release("B", vec![("B", Released)]);

	// Test key combo 1
	test.click("B", vec![("B", Clicked)]);
	test.click("A", vec![("A", Clicked)]);
	test.press("A", vec![("A", Pressed)]);
	test.release("B", vec![("B", Released)]);
	test.release("A", vec![("A", Released)]);

	// Test simple mapping
	test.click("A", vec![("A", Clicked)]);
	test.release("A", vec![("A", Released)]);
	test.click("3", vec![("2", Clicked)]);
	test.press("3", vec![("2", Pressed)]);
	test.release("3", vec![("2", Released)]);

	// Test complex mapping
	test.click("A", vec![("A", Clicked)]);
	test.click("1", vec![("A", ForceReleased), ("3", Clicked)]);
	test.release("1", vec![("3", Released)]);
	test.click("1", vec![("3", Clicked)]);
	test.click("3", vec![("A", Clicked), ("2", Clicked)]);
	// test.assert_click("4", vec![("4", Clicked)]);
	// test.assert_release("A", vec![]);
	// test.assert_release("1", vec![("3", Released)]);

	Ok(())
}

#[test]
fn test_1() {
	let mut kbct = KbctTestContext::new(
		hashmap! {"1" => "2", "3" => "4"},
		hashmap! {
		btreeset! {"A"} => hashmap!{"1" => "3"},
		btreeset! {"A", "B"} => hashmap! {"1" => "5"}
		},
	);

	kbct.click("A", vec![("A", Clicked)]);
	kbct.click("1", vec![("A", ForceReleased), ("3", Clicked)]);
	kbct.release("1", vec![("3", Released)]);
	kbct.click("B", vec![("A", Clicked), ("B", Clicked)]);
	kbct.click(
		"1",
		vec![("A", ForceReleased), ("B", ForceReleased), ("5", Clicked)],
	);
}

#[test]
fn test_2() {
	let mut kbct = KbctTestContext::new(
		hashmap! {"1" => "2", "3" => "4", "A" => "C"},
		hashmap! {
		btreeset! {"A"} => hashmap!{"1" => "3"},
		btreeset! {"A", "B"} => hashmap! {"1" => "5"}
		},
	);

	kbct.click("A", vec![("C", Clicked)]);
	kbct.click("1", vec![("C", ForceReleased), ("3", Clicked)]);
	kbct.release("1", vec![("3", Released)]);
	kbct.click("B", vec![("C", Clicked), ("B", Clicked)]);
	kbct.click(
		"1",
		vec![("C", ForceReleased), ("B", ForceReleased), ("5", Clicked)],
	);
}

#[test]
fn test_3() {
	let mut kbct = KbctTestContext::new(
		hashmap! {
		"X" => "Z",
		"A" => "B",
		"C" => "K"},
		hashmap! {
		btreeset! {"A"} => hashmap!{"X" => "Y"},
		btreeset! {"A", "C"} => hashmap! {"X" => "T"}},
	);

	kbct.click("A", vec![("B", Clicked)]);
	kbct.click("X", vec![("B", ForceReleased), ("Y", Clicked)]);
	kbct.release("X", vec![("Y", Released)]);
	kbct.click("C", vec![("B", Clicked), ("K", Clicked)]);
	kbct.click(
		"X",
		vec![("B", ForceReleased), ("K", ForceReleased), ("T", Clicked)],
	);
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
	let kbct = Kbct::new(
		KbctConf {
			keyboards: vec![],
			keymap: Some(map_string(hashmap!["C" => "D"])),
			layers: Some(vec![
				KbctComplexConf {
					modifiers: vec!["A".to_string(), "B".to_string()],
					keymap: map_string(hashmap!["1" => "2", "2" => "1"]),
				},
				KbctComplexConf {
					modifiers: vec!["A".to_string()],
					keymap: map_string(hashmap!["1" => "3"]),
				},
			]),
		},
		|_| None,
	);
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
fn test_create_simple_kbct() -> Result<()> {
	let simple = map_string(hashmap! {
		"K1" => "K2"
	});
	let kbct = Kbct::new(
		KbctConf {
			keyboards: vec![],
			keymap: Some(simple),
			layers: None,
		},
		create_keymap_func(|x| match x {
			"K1" => 1,
			"K2" => 2,
			_ => -1,
		}),
	)?;
	assert_eq!(1, kbct.simple_map.len());
	assert_eq!(2, kbct.simple_map.get(&1).unwrap().code);
	Ok(())
}

#[test]
fn test_conf_parser() -> Result<()> {
	let yml = "keyboards: []\nkeymap:";
	let conf = KbctConf::parse(yml.to_string())?;
	assert_eq!(None, conf.keymap);
	assert_eq!(None, conf.layers);

	let yml = "keyboards: []\nignored_key: 12";
	let conf = KbctConf::parse(yml.to_string())?;
	assert_eq!(None, conf.keymap);
	assert_eq!(None, conf.layers);

	let yml = "keyboards: []\nkeymap:\n  KEY: VALUE\n";
	let conf = KbctConf::parse(yml.to_string())?;
	assert!(conf.keymap.is_some());
	assert_eq!(None, conf.layers);
	let map = conf.keymap.unwrap();
	assert_eq!(
		&KeyPressConf::Key("VALUE".to_string()),
		map.get("KEY").unwrap()
	);
	assert_eq!(1, map.len());

	let yml = "keyboards: []\nlayers:\n- modifiers: ['LEFT_ALT']\n  keymap:\n    KEY_I: UP_ARROW";
	let conf = KbctConf::parse(yml.to_string())?;
	assert!(conf.layers.is_some());
	let complex_vec = conf.layers.unwrap();
	let conf = complex_vec.get(0).unwrap();
	assert_eq!(
		&KeyPressConf::Key("UP_ARROW".to_string()),
		conf.keymap.get("KEY_I").unwrap()
	);
	assert_eq!(1, conf.keymap.len());
	assert_eq!(1, conf.modifiers.len());
	assert_eq!("LEFT_ALT", conf.modifiers.first().unwrap());

	let yml = "keyboards: []\nlayers:\n  modifiers: ['LEFT_ALT']";
	let conf = KbctConf::parse(yml.to_string());
	assert!(conf.is_err());

	let yml = "keyboards: []\nlayers:\n  keymap:\n    KEY_I: UP_ARROW";
	let conf = KbctConf::parse(yml.to_string());
	assert!(conf.is_err());

	Ok(())
}
