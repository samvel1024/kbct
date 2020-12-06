#![allow(unused_imports)]
#![allow(dead_code)]
extern crate chrono;
#[macro_use]
extern crate log;
extern crate pretty_env_logger;
extern crate uinput;
extern crate uinput_sys;
#[macro_use]
extern crate maplit;

use std::{fs::File, io::{self}, process};
use std::collections::HashMap;
use std::env;
use std::fs::OpenOptions;
use std::io::{Error, ErrorKind, Read};
use std::mem;
use std::os::unix::io::AsRawFd;

use inotify::Inotify;
use ioctl_rs;
use mio::{Events, Interest, Poll, Token};
use mio::event::Event;
use mio::unix::SourceFd;
use nix::sys::signal::SigSet;
use nix::sys::signalfd::SignalFd;
use uinput::Device;
use uinput_sys::*;
use uinput_sys::input_event;

struct EventLoop {
	events: Events,
	poll: Poll,
	running: bool,
	handlers: HashMap<Token, Box<dyn EventObserver>>,
}

enum ObserverResult {
	Nothing,
	Unsubcribe,
	Terminate {
		status: i32
	},
	SubscribeNew(Box<dyn EventObserver>),
}

impl EventLoop {
	fn run(&mut self) -> io::Result<()> {
		while self.running {
			self.poll.poll(&mut self.events, None)?;
			for ev in self.events.iter() {
				let handler = self.handlers.get_mut(&ev.token()).unwrap();
				match handler.on_event(ev)? {
					ObserverResult::Nothing => {}
					ObserverResult::Unsubcribe => { unimplemented!() }
					ObserverResult::Terminate { status: _status } => {
						self.running = false;
					}
					ObserverResult::SubscribeNew(_x) => { unimplemented!() }
				}
			}
		}
		Ok(())
	}

	fn register_observer(&mut self, fd: i32, token: Token, obs: Box<dyn EventObserver>) -> io::Result<()> {
		self.poll.registry().register(&mut SourceFd(&fd), token, Interest::READABLE)?;
		if self.handlers.contains_key(&token) {
			Err(Error::new(ErrorKind::AlreadyExists, "The handler token already registered"))
		} else {
			self.handlers.insert(token, obs);
			Ok(())
		}
	}
}

trait EventObserver {
	fn on_event(&mut self, _: &Event) -> io::Result<ObserverResult>;
}

struct SignalReceiver {
	signal_fd: SignalFd,
}

impl SignalReceiver {
	fn register(evloop: &mut EventLoop) -> io::Result<()> {
		let mut mask = SigSet::empty();
		mask.add(nix::sys::signal::SIGTERM);
		mask.add(nix::sys::signal::SIGINT);
		mask.thread_block().unwrap();
		const SIG_EVENT: Token = Token(1);
		let sfd = nix::sys::signalfd::SignalFd::with_flags(
			&mask, nix::sys::signalfd::SfdFlags::SFD_NONBLOCK).unwrap();
		evloop.register_observer(sfd.as_raw_fd(), SIG_EVENT, Box::new(SignalReceiver { signal_fd: (sfd) }))?;
		trace!("Registered SIGTERM, SIGINT handlers");
		Ok(())
	}
}

impl EventObserver for SignalReceiver {
	fn on_event(&mut self, _: &Event) -> io::Result<ObserverResult> {
		info!("Received signal, stopping");
		Ok(ObserverResult::Terminate {
			status: 0
		})
	}
}

struct KeyboardMapper {
	file: File,
	current_layer: i32,
	layers: [i32; 1024],
	device: Device,
	raw_buffer: [u8; KeyboardMapper::BUF_SIZE],
}

impl KeyboardMapper {
	const MAX_EVS: usize = 1024;
	const BUF_SIZE: usize = mem::size_of::<input_event>() * KeyboardMapper::MAX_EVS;

	fn register(evloop: &mut EventLoop, dev_file: String) -> io::Result<()> {
		let kb_mapper = Box::new(KeyboardMapper {
			file: OpenOptions::new()
				.read(true)
				.write(false)
				.open(dev_file)?,
			current_layer: 0,
			layers: [-1; 1024],
			device: KeyboardMapper::open_uinput_device()?,
			raw_buffer: [0; KeyboardMapper::BUF_SIZE],
		});
		kb_mapper.grab_keyboard()?;
		const DEVICE_EVENT: Token = Token(0);
		evloop.register_observer(kb_mapper.file.as_raw_fd(),
														 DEVICE_EVENT,
														 kb_mapper)
	}

	fn open_uinput_device() -> io::Result<uinput::Device> {
		let mut builder = uinput::default().unwrap()
			.name("test").unwrap()
			.event(uinput::event::Keyboard::All).unwrap()
			.event(uinput::event::Controller::All).unwrap();

		for item in uinput::event::relative::Position::iter_variants() {
			builder = builder.event(item).unwrap();
		}

		for item in uinput::event::relative::Wheel::iter_variants() {
			builder = builder.event(item).unwrap();
		}
		Ok(builder.create().unwrap())
	}

	fn grab_keyboard(&self) -> Result<(), Error> {
		info!("Trying to grab device {:?}", self.file);
		const EVIOCGRAB: u32 = 1074021776;
		match unsafe { ioctl_rs::ioctl(self.file.as_raw_fd(), EVIOCGRAB, 1) } {
			0 => Ok(()),
			_ => Err(Error::last_os_error()),
		}
	}

	fn map(&mut self, ev: &mut input_event) {
		let code = ev.code as usize;

		if ev.value == 1 {
			self.layers[code] = self.current_layer;
		}
		if self.layers[code] == -1 {
			return;
		}

		ev.code = match (code as i32, self.layers[code]) {
			(KEY_RIGHTCTRL, 0) => KEY_RIGHTMETA,
			(KEY_LEFTALT, 0) => KEY_LEFTCTRL,
			(KEY_CAPSLOCK, 0) => KEY_LEFTALT,
			(KEY_LEFTCTRL, 0) => KEY_RIGHTALT,
			(KEY_SYSRQ, 0) => KEY_RIGHTCTRL,

			(KEY_L, 1) => KEY_RIGHT,
			(KEY_I, 1) => KEY_UP,
			(KEY_J, 1) => KEY_LEFT,
			(KEY_K, 1) => KEY_DOWN,
			(KEY_U, 1) => KEY_PAGEUP,
			(KEY_O, 1) => KEY_PAGEDOWN,
			(KEY_SEMICOLON, 1) => KEY_END,
			(KEY_P, 1) => KEY_HOME,
			(KEY_N, 1) => KEY_INSERT,
			(KEY_COMMA, 1) => KEY_COMPOSE,

			(KEY_1, 1) => KEY_F1,
			(KEY_2, 1) => KEY_F2,
			(KEY_3, 1) => KEY_F3,
			(KEY_4, 1) => KEY_F4,
			(KEY_5, 1) => KEY_F5,
			(KEY_6, 1) => KEY_F6,
			(KEY_7, 1) => KEY_F7,
			(KEY_8, 1) => KEY_F8,
			(KEY_9, 1) => KEY_F9,
			(KEY_MINUS, 1) => KEY_F11,
			(KEY_EQUAL, 1) => KEY_F12,
			(KEY_HOME, 1) => KEY_BRIGHTNESSUP,
			(KEY_F12, 1) => KEY_BRIGHTNESSDOWN,
			(KEY_F11, 1) => KEY_VOLUMEUP,
			(KEY_F10, 1) => KEY_VOLUMEDOWN,
			(KEY_F9, 1) => KEY_MUTE,
			(KEY_RIGHTSHIFT, 1) => KEY_CAPSLOCK,
			(KEY_RIGHTBRACE, 1) => KEY_NEXTSONG,
			(KEY_LEFTBRACE, 1) => KEY_PREVIOUSSONG,
			(KEY_BACKSLASH, 1) => KEY_PLAYPAUSE,

			(KEY_H, 1) => KEY_MENU,
			(KEY_Y, 1) => KEY_PROG4,
			(KEY_M, 1) => KEY_BACKSPACE,
			(KEY_DOT, 1) => KEY_DELETE,
			(KEY_LEFTALT, 1) => KEY_LEFTCTRL,
			(KEY_CAPSLOCK, 1) => KEY_LEFTALT,
			(KEY_LEFTCTRL, 1) => KEY_RIGHTALT,
			(KEY_SYSRQ, 1) => KEY_RIGHTCTRL,
			(left, _) => left,
		} as u16;
	}
}

impl EventObserver for KeyboardMapper {
	fn on_event(&mut self, _: &Event) -> io::Result<ObserverResult> {
		// trace!("vent")
		let events_count = self.file.read(&mut self.raw_buffer)? / mem::size_of::<input_event>();
		let mut events = unsafe {
			mem::transmute::<[u8; KeyboardMapper::BUF_SIZE], [input_event; KeyboardMapper::MAX_EVS]>(self.raw_buffer)
		};
		for i in 0..events_count {
			let mut skip = false;
			if events[i].kind == EV_KEY as u16 {
				if events[i].code as i32 == KEY_RIGHTALT {
					skip = true;
					if events[i].value == 0 {
						self.current_layer = 0;
					} else {
						self.current_layer = 1;
					}
				} else {
					self.map(&mut events[i]);
				}
			}
			if !skip {
				self.device.write(events[i].kind as i32, events[i].code as i32, events[i].value).unwrap();
			}
		}
		Ok(ObserverResult::Nothing)
	}
}

struct DeviceWatcher {
	inotify: Inotify
}

impl DeviceWatcher {
	fn register(evloop: &mut EventLoop) -> io::Result<()> {
		//Setup inotify poll reader
		let mut watcher = DeviceWatcher {
			inotify: inotify::Inotify::init()
				.expect("Error while initializing inotify instance")
		};
		watcher.inotify
			.add_watch(
				"/dev/input",
				inotify::WatchMask::CREATE | inotify::WatchMask::DELETE,
			)
			.expect("Failed to add file watch");
		const SIG_INOTIFY: Token = Token(2);
		evloop.register_observer(watcher.inotify.as_raw_fd(), SIG_INOTIFY, Box::new(watcher))?;
		Ok(())
	}
}

impl EventObserver for DeviceWatcher {
	fn on_event(&mut self, _: &Event) -> io::Result<ObserverResult> {
		let mut buffer = [0; 1024];
		let events = self.inotify.read_events_blocking(&mut buffer)
			.expect("Error while reading events");
		for event in events {
			if event.mask.contains(inotify::EventMask::CREATE) {
				if event.mask.contains(inotify::EventMask::ISDIR) {
					println!("Directory created: {:?}", event.name);
				} else {
					println!("File created: {:?}", event.name);
				}
			} else if event.mask.contains(inotify::EventMask::DELETE) {
				if event.mask.contains(inotify::EventMask::ISDIR) {
					println!("Directory deleted: {:?}", event.name);
				} else {
					println!("File deleted: {:?}", event.name);
				}
			} else if event.mask.contains(inotify::EventMask::MODIFY) {
				if event.mask.contains(inotify::EventMask::ISDIR) {
					println!("Directory modified: {:?}", event.name);
				} else {
					println!("File modified: {:?}", event.name);
				}
			}
		}
		Ok(ObserverResult::Nothing)
	}
}


fn main() -> io::Result<()> {
	pretty_env_logger::init_timed();
	let program_args: Vec<String> = env::args().collect();


	let mut evloop = EventLoop {
		poll: Poll::new()?,
		events: Events::with_capacity(1024),
		running: true,
		handlers: HashMap::new(),
	};

	SignalReceiver::register(&mut evloop)?;
	KeyboardMapper::register(&mut evloop, program_args[1].clone())?;
	DeviceWatcher::register(&mut evloop)?;

	info!("Starting laykeymap event loop, pid={}", process::id());
	evloop.run()?;

	Ok(())
}
