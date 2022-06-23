use std::fs;

use kbct::Result;
use kbct::{KbctError, KbctEvent, KbctKeyStatus};
use uinput::Device;

extern crate text_io;

use crate::util::keycodes::{code_to_name, name_to_code};
use core::{fmt, mem};
use regex::Regex;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Error, Read};
use std::os::unix::io::AsRawFd;
use uinput_sys::{input_event, EV_KEY};

// ioctl constants obtained from uinput C library
const EVIOCGRAB: u32 = 1074021776;
const EVIOCGNAME_256: u32 = 2164278534;

const MAX_EVS: usize = 256;
pub const BUF_SIZE: usize = mem::size_of::<input_event>() * MAX_EVS;

pub type KeyBuffer = [u8; BUF_SIZE];

pub fn get_uinput_device_name(dev_file_path: &String) -> Result<Option<String>> {
	let file = OpenOptions::new()
		.read(true)
		.write(false)
		.open(&dev_file_path)?;
	let buff = [0u8; 256];
	let str_len = unsafe { ioctl_rs::ioctl(file.as_raw_fd(), EVIOCGNAME_256, &buff) };
	if str_len > 0 {
		Ok(Some(
			std::str::from_utf8(&buff[..(str_len - 1) as usize]).map(|x| x.to_string())?,
		))
	} else {
		Err(KbctError::IOError(Error::last_os_error()))
	}
}

pub fn get_all_uinput_device_names_to_paths() -> Result<HashMap<String, String>> {
	let paths = fs::read_dir("/dev/input/")?;
	let regex: Regex = Regex::new("^.*event\\d+$")?;
	let mut ans = hashmap![];
	for path in paths {
		let path_buf = path?.path();
		let device_path = path_buf.to_string_lossy();
		if regex.is_match(&device_path) {
			if let Some(device_name) = get_uinput_device_name(&device_path.to_string())? {
				ans.insert(device_name, (*device_path.to_string()).to_string());
			}
		}
	}
	Ok(ans)
}

pub fn open_readable_uinput_device(dev_file_path: &String, should_grab: bool) -> Result<File> {
	let file = OpenOptions::new()
		.read(true)
		.write(false)
		.open(&dev_file_path)?;
	if should_grab {
		match unsafe { ioctl_rs::ioctl(file.as_raw_fd(), EVIOCGRAB, 1) } {
			0 => Ok(file),
			_ => Err(KbctError::IOError(Error::last_os_error())),
		}
	} else {
		Ok(file)
	}
}

pub fn linux_keyname_mapper(name: &String) -> Option<i32> {
	match name_to_code(name) {
		-1 => None,
		x => Some(x),
	}
}

pub fn create_writable_uinput_device(name: &String) -> Result<Device> {
	let mut builder = uinput::default()?
		.name(&name)?
		.event(uinput::event::Keyboard::All)?
		.event(uinput::event::Controller::All)?;
	for item in uinput::event::relative::Position::iter_variants() {
		builder = builder.event(item)?;
	}
	for item in uinput::event::relative::Wheel::iter_variants() {
		builder = builder.event(item)?;
	}
	let x = builder.create()?;
	Ok(x)
}

pub fn map_status_from_linux(val: i32) -> KbctKeyStatus {
	match val {
		0 => KbctKeyStatus::Released,
		1 => KbctKeyStatus::Clicked,
		2 => KbctKeyStatus::Pressed,
		_ => panic!("Illegal argument {}", val),
	}
}

pub fn map_status_from_kbct(val: KbctKeyStatus) -> i32 {
	match val {
		KbctKeyStatus::Released | KbctKeyStatus::ForceReleased => 0,
		KbctKeyStatus::Clicked => 1,
		KbctKeyStatus::Pressed => 2,
	}
}

pub fn kbct_from_uinput_event(val: &input_event) -> Option<KbctEvent> {
	if val.kind as i32 == EV_KEY {
		Some(KbctEvent {
			code: val.code as i32,
			ev_type: map_status_from_linux(val.value),
		})
	} else {
		None
	}
}

pub fn read_key_events(file: &mut File, buf: &mut KeyBuffer) -> Result<Vec<input_event>> {
	let bytes_read = file.read(buf)?;
	let event_count = bytes_read / mem::size_of::<input_event>();
	let events = unsafe { mem::transmute::<[u8; BUF_SIZE], [input_event; MAX_EVS]>(*buf) };
	Ok(events[..event_count].to_vec())
}

#[derive(PartialEq, Clone, Debug)]
pub struct KeyEvent {
	pub keycode: i32,
	pub statuscode: i32,
}

impl KeyEvent {
	fn from_kbct_event(ev: &KbctEvent) -> KeyEvent {
		KeyEvent {
			keycode: ev.code,
			statuscode: map_status_from_kbct(ev.ev_type),
		}
	}
}

#[derive(Clone)]
pub struct KeyMapEvent {
	pub input: KeyEvent,
	pub output: Vec<KeyEvent>,
}

impl KeyMapEvent {
	pub fn from_kbct_event(input: KbctEvent, output: &Vec<KbctEvent>) -> KeyMapEvent {
		KeyMapEvent {
			input: KeyEvent::from_kbct_event(&input),
			output: output
				.iter()
				.map(|x| KeyEvent::from_kbct_event(x))
				.collect(),
		}
	}

	fn format_key_event(x: &KeyEvent) -> String {
		let key = code_to_name(x.keycode);
		let status = match x.statuscode {
			1 => "+",
			0 => "-",
			2 => "=",
			_ => panic!("Illegal val"),
		};
		format!("{}{}", status.to_string(), key)
	}
}

impl fmt::Display for KeyMapEvent {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let target_arr: Vec<String> = self
			.output
			.iter()
			.map(|x| KeyMapEvent::format_key_event(x))
			.collect();
		let target_str = target_arr.join(" ");
		write!(
			f,
			"{} -> {}",
			KeyMapEvent::format_key_event(&self.input),
			target_str
		)
	}
}
