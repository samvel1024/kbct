
use kbct::Result;
use kbct::KbctError;
use crate::util::*;
use crate::util::keycodes::*;
use core::{fmt, mem, time};
use std::fs::File;
use std::sync::mpsc::{Receiver, Sender, channel};
use uinput_sys::{input_event, EV_KEY};
use mio::unix::SourceFd;
use mio::{Interest, Token};
use std::time::Duration;
use std::io::{Read, BufRead};
use std::{thread, io};
use regex::Regex;
use std::os::unix::io::AsRawFd;
use crate::util::{KeyMapEvent, KeyEvent};

#[derive(PartialEq)]
enum ReplayMessage {
	MappedResult(Vec<KeyEvent>),
	WaitForAssert,
	Finish,
}


fn parse_test_case(line: &str, line_number: i32) -> KeyMapEvent {
	fn parse_key(str: &str) -> KeyEvent {
		let first = str.chars().nth(0).unwrap();
		let statuscode = match first {
			'+' => 1,
			'=' => 2,
			'-' => 0,
			_ => panic!("Illegal state")
		};
		let keycode = name_to_code(&str[1..]);
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
	KeyMapEvent {
		input: left,
		output: right,
	}
}

fn read_keyboard_output(mut device_file: File, receiver: Receiver<ReplayMessage>, sender: Sender<ReplayMessage>) -> Result<()> {
	use ReplayMessage::*;
	const MAX_EVENTS: usize = 1024;
	const BUF_SIZE: usize = mem::size_of::<input_event>() * MAX_EVENTS;
	const TIMEOUT: u64 = 50;

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


pub fn replay(test_file: String) -> Result<()> {
	use ReplayMessage::*;
	use kbct::KbctKeyStatus::*;

	let device_name = "DummyDevice".to_string();
	let mut device = create_writable_uinput_device(&device_name)?;

	// Allow some time for the kbct process to capture the new device
	thread::sleep(time::Duration::from_millis(800));

	let all_devices = get_all_uinput_device_names_to_paths()?;
	let mapped_device_path = all_devices.get("Kbct-DummyDevice")
		.expect("The mapped device is not mounted yet, make sure you run kbct in parallel before replay");
	let mapped_device_file = open_readable_uinput_device(mapped_device_path, true)?;

	let lines = {
		let file = File::open(test_file)?;
		io::BufReader::new(file).lines()
	};
	let (send_wait_for_assert, recv) = channel();
	let (send_wait_for_key, receive_wait_for_key) = channel();
	let thread = thread::spawn(move || {
		read_keyboard_output(mapped_device_file, recv, send_wait_for_key).unwrap();
	});


	let mut line_number = 1;
	for line in lines {
		if let Ok(line) = line {
			if line.trim().is_empty() || line.trim().starts_with("#") {
				line_number += 1;
				continue;
			}
			let ev = parse_test_case(&line.as_str(), line_number);
			device.write(EV_KEY, ev.input.keycode, ev.input.statuscode)?;
			device.synchronize()?;


			send_wait_for_assert.send(WaitForAssert).expect("Could not send");
			match receive_wait_for_key.recv().expect("Coult not receive") {
				MappedResult(result) => {
					let expected_str = format!("{}", ev);
					let actual = KeyMapEvent {
						input: ev.input,
						output: result,
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
	info!("Test passed");

	match thread.join() {
		Ok(_) => Ok(()),
		Err(_) => Err(KbctError::Error("Error joining thread".to_string())),
	}
}


#[cfg(test)]
mod tests {
	#[test]
	fn test_parse_test_case() {
		fn t(s: &str) -> String {
			format!("{}", crate::util::integration_test::parse_test_case(s, 1))
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
