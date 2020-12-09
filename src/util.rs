#![allow(unused_imports)]
#![allow(dead_code)]


mod keycodes;
mod nio;

use std::{thread, time, io};

use uinput::Device;
use uinput::device::Builder;
use uinput::event::keyboard;
use clap::Clap;
use kbct::{KbctEvent, KbctKeyStatus, Kbct, KbctError};
use kbct::Result;
use thiserror::Error;

extern crate text_io;

use text_io::read;
use std::str;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, Read, Error, BufReader, Lines, stdin};
use uinput_sys::{EV_KEY, input_event};
use ioctl_rs::ioctl;
use std::process::Command;
use kbct::KbctError::IOError;
use std::os::unix::io::{AsRawFd, RawFd};
use crate::nio::{EventObserver, ObserverResult, EventLoop};
use mio::event::Event;
use std::path::Iter;
use mio::{Token, Interest};
use std::time::Duration;
use core::{mem, fmt};
use crate::keycodes::code_to_name;
use std::sync::{Mutex, Arc};
use std::sync::mpsc::{channel, Receiver, Sender};
use mio::unix::SourceFd;
use crate::ReplayMessage::MappedResult;
use regex::Regex;


#[derive(Clap)]
struct CliRoot {
	#[clap(subcommand)]
	subcmd: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
	#[clap()]
	Replay(CliReplay)
}

#[derive(Clap)]
struct CliReplay {
	#[clap(short, long)]
	testcase: String,
	#[clap(short, long)]
	config: String,
}


fn grab_device_file(dev_file: &String) -> Result<File> {
	let file = format!("/dev/input/{}", dev_file);

	let file = OpenOptions::new()
		.read(true)
		.write(false)
		.open(&file)
		.expect(format!("Could not open file {}", file).as_str());
	const EVIOCGRAB: u32 = 1074021776;
	match unsafe { ioctl_rs::ioctl(file.as_raw_fd(), EVIOCGRAB, 1) } {
		0 => Ok(file),
		_ => Err(IOError(Error::last_os_error())),
	}
}

fn get_device_path(name: String) -> String {
	let raw = Command::new("bash")
		.arg("-c")
		.arg(format!(
			"cat /proc/bus/input/devices | grep -A 5 -B 2 {} | grep Handlers | grep -oE 'event[0-9]+'",
			name)
		)
		.output()
		.expect("Failed to get")
		.stdout;
	String::from_utf8_lossy(&raw).to_string().trim().to_string()
}

fn open_uinput_device() -> Result<(Device, String)> {
	let name = "KbctTest".to_string();
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
	Ok((x, get_device_path(name)))
}

#[derive(PartialEq)]
enum ReplayMessage {
	MappedResult(Vec<KeyEvent>),
	WaitForAssert,
	Finish,
}

#[derive(PartialEq, Clone)]
struct KeyEvent {
	keycode: i32,
	statuscode: i32,
}

#[derive(Clone)]
struct TestCase {
	source: KeyEvent,
	expected: Vec<KeyEvent>,
}


impl TestCase {
	fn format_key_event(x: &KeyEvent) -> String {
		let key = keycodes::code_to_name(x.keycode).to_string();
		let status = match x.statuscode {
			1 => "DOWN",
			0 => "UP",
			2 => "PRESS",
			_ => panic!("Illegal val")
		};
		format!("({}, {})", key, status.to_string())
	}
}

impl fmt::Display for TestCase {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let target_arr: Vec<String> = self.expected.iter()
			.map(|x| TestCase::format_key_event(x))
			.collect();
		let target_str = target_arr.join(",");
		write!(f, "TestCase{{from:{}, to:[{}]}}", TestCase::format_key_event(
			&self.source), target_str)
	}
}

fn parse_test_case(line: &str, line_number: i32) -> TestCase {
	fn parse_key(str: &str) -> KeyEvent {
		let first = str.chars().nth(0).unwrap();
		let statuscode = match first {
			'+' => 1,
			'=' => 2,
			'-' => 0,
			_ => panic!("Illegal state")
		};
		let keycode = keycodes::name_to_code(format!("KEY_{}", &str[1..].to_uppercase()).as_str());
		KeyEvent {
			keycode,
			statuscode,
		}
	}

	// examples "+a -> ", "+a -> +b", "+a -> -b =c  "
	let regex: Regex = Regex::new(
		r"^([+-=][0-9a-z_]+)\s*->\s*([+-=][0-9a-z_]+(\s+[+-=][0-9a-z_]+)*)*\s*$").unwrap();
	assert!(regex.is_match(line), "Illegal test case on line {}", line_number);
	let caps = regex.captures(line).unwrap();
	let left = parse_key(caps.get(1).map(|x| x.as_str()).unwrap().trim());
	let right: Vec<KeyEvent> = caps.get(2).map(|x| x.as_str().trim()).unwrap_or("")
		.split(" ")
		.filter(|x| !x.is_empty())
		.map(|x| parse_key(x.trim()))
		.collect();
	TestCase {
		source: left,
		expected: right,
	}
}

fn read_keyboard_output(mut device_file: File, receiver: Receiver<ReplayMessage>, sender: Sender<ReplayMessage>) -> Result<()> {
	use ReplayMessage::*;
	const MAX_EVENTS: usize = 1024;
	const BUF_SIZE: usize = mem::size_of::<input_event>() * MAX_EVENTS;
	const TIMEOUT: u64 = 5;

	let fd = device_file.as_raw_fd();
	let mut events = mio::Events::with_capacity(1);
	let mut poll = mio::Poll::new()?;
	let token = Token(0);
	poll.registry().register(&mut SourceFd(&fd), token, Interest::READABLE).unwrap();

	let mut raw_buffer: [u8; BUF_SIZE] = [0; BUF_SIZE];
	loop {
		match receiver.recv().expect("Could not receive") {
			MappedResult(_) => panic!("Received illegal value"),
			Finish => break,
			WaitForAssert => {
				let mut answer = vec![];
				loop {
					poll.poll(&mut events, Some(Duration::from_millis(TIMEOUT))).unwrap();
					if !events.iter().any(|x| x.is_readable()) {
						break; // Timeout happened
					}

					let events_count = device_file.read(&mut raw_buffer)? / mem::size_of::<input_event>();
					let events = unsafe {
						mem::transmute::<[u8; BUF_SIZE], [input_event; MAX_EVENTS]>(raw_buffer)
					};
					for i in 0..events_count {
						let x = &events[i];
						if x.kind == EV_KEY as u16 {
							answer.push(KeyEvent { keycode: x.code as i32, statuscode: x.value })
						}
					}
				}
				sender.send(ReplayMessage::MappedResult(answer)).expect("Could not send");
			}
		}
	}
	Ok(())
}


fn replay(test_file: String, kbct_config_file: String) -> Result<()> {
	use ReplayMessage::*;
	use kbct::KbctKeyStatus::*;

	let (mut device, device_name) = open_uinput_device()?;
	let device_file = grab_device_file(&device_name)?;

	let lines = {
		let file = File::open(test_file)?;
		io::BufReader::new(file).lines()
	};
	let (send_wait_for_assert, recv) = channel();
	let (send_wait_for_key, receive_wait_for_key) = channel();
	let thread = thread::spawn(move || {
		read_keyboard_output(device_file, recv, send_wait_for_key).unwrap();
	});

	let mut kbct = {
		let config = std::fs::read_to_string(kbct_config_file)
			.expect("Could not open config yaml file");
		let conf = kbct::KbctRootConf::parse(config)
			.expect("Illegal config yaml file");
		Kbct::new(conf, |x| match keycodes::name_to_code(format!("KEY_{}", x.to_uppercase()).as_str()) {
			-1 => None,
			x => Some(x)
		}).unwrap()
	};

	let mut line_number = 1;
	for line in lines {
		if let Ok(line) = line {
			if line.trim().is_empty() || line.trim().starts_with("#") {
				line_number += 1;
				continue;
			}
			let ev = parse_test_case(&line.as_str(), line_number);
			let mapping = kbct.map_event(KbctEvent {
				code: ev.source.keycode,
				ev_type: match ev.source.statuscode {
					1 => Clicked,
					2 => Pressed,
					0 => Released,
					_ => panic!("Illegal value")
				},
			});
			for ev in mapping {
				let status = match ev.ev_type {
					Clicked => 1,
					Pressed => 2,
					ForceReleased | Released => 0,
				};
				device.write(EV_KEY, ev.code, status)?;
			}
			device.synchronize()?;


			send_wait_for_assert.send(WaitForAssert).expect("Could not send");
			match receive_wait_for_key.recv().expect("Coult not receive") {
				MappedResult(result) => {
					let expected_str = format!("{}", ev);
					let actual = TestCase {
						source: ev.source,
						expected: result,
					};
					let actual_str = format!("{}", actual);
					assert_eq!(expected_str, actual_str, "Wrong output on line {}", line_number);
				}
				_ => panic!("Received illegal value"),
			}
			line_number += 1;
		}
	}
	send_wait_for_assert.send(Finish).unwrap();
	match thread.join() {
		Ok(_) => Ok(()),
		Err(_) => Err(KbctError::Error("Error joining thread".to_string())),
	}
}

fn main() -> Result<()> {
	let root_opts: CliRoot = CliRoot::parse();

	match root_opts.subcmd {
		SubCommand::Replay(r) => {
			replay(r.testcase, r.config)?;
		}
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	#[test]
	fn test_parse_test_case() {
		fn t(s: &str) -> String {
			format!("{}", crate::parse_test_case(s, 1))
		}
		assert_eq!(
			"TestCase{from:(KEY_A, DOWN), to:[(KEY_B, DOWN)]}",
			t("+a -> +b")
		);
		assert_eq!(
			"TestCase{from:(KEY_LEFTCTRL, UP), to:[]}",
			t("-leftctrl ->    ")
		);
		assert_eq!(
			"TestCase{from:(KEY_A, UP), to:[(KEY_D, DOWN),(KEY_R, UP),(KEY_R, PRESS)]}",
			t("-a ->  +d -r   =r  ")
		);
	}
}
