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
use clap::Clap;

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
use kbct::{Kbct, KbctRootConf, KbctEvent, Result, KbctError};
use std::sync::atomic::Ordering::Release;
use uinput::event::keyboard::Function::Press;
use kbct::KbctKeyStatus::*;
use nio::*;

struct SignalReceiver {
	signal_fd: SignalFd,
}

impl SignalReceiver {
	fn register(evloop: &mut EventLoop) -> Result<()> {
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
	fn on_event(&mut self, _: &Event) -> Result<ObserverResult> {
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

	fn register(evloop: &mut EventLoop, dev_file: String, config_file: String) -> Result<()> {
		let kbct_conf_yaml = std::fs::read_to_string(config_file.as_str())
			.expect("Could not open config yaml file");
		let kbct_conf = KbctRootConf::parse(kbct_conf_yaml)
			.expect("Could not parse yaml file");
		let kbct = Kbct::new(
			kbct_conf,
			|x| match util::keycodes::name_to_code(format!("KEY_{}", x.to_uppercase()).as_str()) {
				-1 => None,
				x => Some(x)
			}).expect("Could not create kbct instance");

		let kb_mapper = Box::new(KeyboardMapper {
			file: util::open_readable_uinput_device(&dev_file, true)?,
			device: util::create_writable_uinput_device(&"KbctCustomisedDevice".to_string())?,
			raw_buffer: [0; KeyboardMapper::BUF_SIZE],
			kbct,
		});

		const DEVICE_EVENT: Token = Token(0);
		evloop.register_observer(kb_mapper.file.as_raw_fd(),
														 DEVICE_EVENT,
														 kb_mapper)
	}
}

impl EventObserver for KeyboardMapper {
	fn on_event(&mut self, _: &Event) -> Result<ObserverResult> {
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
					self.device.write(EV_KEY, x.code, value)?;
				}
			} else {
				self.device.write(x.kind as i32, x.code as i32, x.value)?;
			}
		}
		Ok(ObserverResult::Nothing)
	}
}

struct DeviceWatcher {
	inotify: Inotify
}

impl DeviceWatcher {
	fn register(evloop: &mut EventLoop) -> Result<()> {
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
	fn on_event(&mut self, _: &Event) -> Result<ObserverResult> {
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

fn start_mapper(config_file: String) -> Result<()> {
	pretty_env_logger::init_timed();
	let mut evloop = EventLoop::new()?;
	SignalReceiver::register(&mut evloop)?;
	KeyboardMapper::register(&mut evloop, "/dev/input/event2".to_string(), config_file)?;
	DeviceWatcher::register(&mut evloop)?;
	println!("Starting...");
	info!("Starting kbct event loop, pid={}", process::id());
	evloop.run()?;

	Ok(())
}


#[derive(Clap)]
struct CliRoot {
	#[clap(subcommand)]
	subcmd: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
	#[clap()]
	TestReplay(CliTestReplay),
	#[clap()]
	Remap(CliRemap),
}

#[derive(Clap)]
struct CliTestReplay {
	#[clap(short, long)]
	testcase: String,
	#[clap(short, long)]
	config: String,
}

#[derive(Clap)]
struct CliRemap {
	#[clap(short, long)]
	config: String,
}

fn main() -> Result<()> {
	let root_opts: CliRoot = CliRoot::parse();
	use SubCommand::*;
	match root_opts.subcmd {
		TestReplay(args) => {
			util::replay(args.testcase, args.config)?;
		}
		Remap(args) => {
			start_mapper(args.config)?;
		}
	}
	Ok(())
}

mod nio;
mod util;
