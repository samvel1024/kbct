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
use kbct::{Kbct, KbctRootConf, KbctEvent};
use std::sync::atomic::Ordering::Release;
use uinput::event::keyboard::Function::Press;
use kbct::KbctKeyStatus::*;

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
	device: Device,
	raw_buffer: [u8; KeyboardMapper::BUF_SIZE],
	kbct: Kbct,
}

impl KeyboardMapper {
	const MAX_EVS: usize = 1024;
	const BUF_SIZE: usize = mem::size_of::<input_event>() * KeyboardMapper::MAX_EVS;

	fn register(evloop: &mut EventLoop, dev_file: String) -> io::Result<()> {
		let kbct_conf_yaml = std::fs::read_to_string("./conf.yaml")
			.expect("Could not open config yaml file");
		let kbct_conf = KbctRootConf::parse(kbct_conf_yaml)
			.expect("Could not parse yaml file");
		let kbct = Kbct::new(
			kbct_conf,
			|x| match keycodes::name_to_code(format!("KEY_{}", x.to_uppercase()).as_str()) {
				-1 => None,
				x => Some(x)
			}).expect("Could not create kbct instance");

		let kb_mapper = Box::new(KeyboardMapper {
			file: OpenOptions::new()
				.read(true)
				.write(false)
				.open(dev_file)?,
			device: KeyboardMapper::open_uinput_device()?,
			raw_buffer: [0; KeyboardMapper::BUF_SIZE],
			kbct,
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
}

impl EventObserver for KeyboardMapper {
	fn on_event(&mut self, _: &Event) -> io::Result<ObserverResult> {
		// trace!("vent")
		let events_count = self.file.read(&mut self.raw_buffer)? / mem::size_of::<input_event>();
		let events = unsafe {
			mem::transmute::<[u8; KeyboardMapper::BUF_SIZE], [input_event; KeyboardMapper::MAX_EVS]>(self.raw_buffer)
		};

		for i in 0..events_count {
			let x = events[i];
			if events[i].kind == EV_KEY as u16 {
				let ev = match events[i].value {
					0 => Released,
					2 => Pressed,
					1 => Clicked,
					_ => panic!("Unknown event value")
				};
				let result = self.kbct.map_event(KbctEvent { code: events[i].code as i32, ev_type: ev });
				for x in result {
					println!("Mapped {:?}", x);
					let value = match x.ev_type {
						Released | ForceReleased => 0,
						Pressed => 2,
						Clicked => 1,
					};
					self.device.write(EV_KEY, x.code, value).unwrap();
				}
			} else {
				self.device.write(x.kind as i32, x.code as i32, x.value);
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
	let mut evloop = EventLoop {
		poll: Poll::new()?,
		events: Events::with_capacity(1024),
		running: true,
		handlers: HashMap::new(),
	};

	SignalReceiver::register(&mut evloop)?;
	KeyboardMapper::register(&mut evloop, "/dev/input/event2".to_string())?;
	DeviceWatcher::register(&mut evloop)?;
	println!("Starting...");
	info!("Starting laykeymap event loop, pid={}", process::id());
	evloop.run()?;

	Ok(())
}

mod keycodes;