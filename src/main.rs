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

use std::{fs::File, io::{self}, process, fs, thread, time};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::OpenOptions;
use std::io::{Error, ErrorKind, Read};
use std::mem;
use std::os::unix::io::{AsRawFd, RawFd};
use clap::Clap;

use inotify::Inotify;
use ioctl_rs;
use mio::event::{Event, Source};
use nix::sys::signal::SigSet;
use nix::sys::signalfd::SignalFd;
use uinput::Device;
use uinput_sys::*;
use uinput_sys::input_event;
use kbct::*;
use std::sync::atomic::Ordering::Release;
use uinput::event::keyboard::Function::Press;
use kbct::KbctKeyStatus::*;
use nio::*;
use regex::Regex;
use crate::util::get_uinput_device_name;
use mio::{event, Registry, Token, Interest};
use mio::unix::SourceFd;

struct SignalReceiver {
	signal_fd: SignalFd,
	raw_fd: RawFd,
}

impl SignalReceiver {
	fn new() -> Result<Box<SignalReceiver>> {
		let mut mask = SigSet::empty();
		mask.add(nix::sys::signal::SIGTERM);
		mask.add(nix::sys::signal::SIGINT);
		mask.thread_block().unwrap();
		let sfd = nix::sys::signalfd::SignalFd::with_flags(
			&mask, nix::sys::signalfd::SfdFlags::SFD_NONBLOCK).unwrap();
		let fd = sfd.as_raw_fd();
		Ok(Box::new(SignalReceiver { signal_fd: (sfd), raw_fd: fd }))
	}
}


impl EventObserver for SignalReceiver {
	fn on_event(&mut self, _: &Event) -> Result<ObserverResult> {
		info!("Received signal, stopping");
		Ok(ObserverResult::Terminate {
			status: 0
		})
	}

	fn get_source_fd(&self) -> SourceFd {
		SourceFd(&self.raw_fd)
	}
}

struct KeyboardMapper {
	file: File,
	device: Device,
	raw_buffer: [u8; KeyboardMapper::BUF_SIZE],
	kbct: Kbct,
	raw_fd: RawFd,
}

impl KeyboardMapper {
	const MAX_EVS: usize = 1024;
	const BUF_SIZE: usize = mem::size_of::<input_event>() * KeyboardMapper::MAX_EVS;

	fn new(dev_file: String, config_file: String) -> Result<Box<KeyboardMapper>> {
		let kbct_conf_yaml = std::fs::read_to_string(config_file.as_str())
			.expect("Could not open config yaml file");
		let kbct_conf = KbctConf::parse(kbct_conf_yaml)
			.expect("Could not parse yaml file");

		let kbct = Kbct::new(
			kbct_conf,
			|x| match util::keycodes::name_to_code(format!("KEY_{}", x.to_uppercase()).as_str()) {
				-1 => None,
				x => Some(x)
			}).expect("Could not create kbct instance");

		let file = util::open_readable_uinput_device(&dev_file, true)?;
		let raw_fd = file.as_raw_fd();
		let device = util::create_writable_uinput_device(&"KbctCustomisedDevice".to_string())?;
		let raw_buffer = [0; KeyboardMapper::BUF_SIZE];

		let kb_mapper = Box::new(KeyboardMapper {
			file,
			device,
			raw_buffer,
			kbct,
			raw_fd,
		});
		Ok(kb_mapper)
	}
}

impl EventObserver for KeyboardMapper {
	fn on_event(&mut self, _: &Event) -> Result<ObserverResult> {
		// trace!("vent")
		if let Ok(bytes_read) = self.file.read(&mut self.raw_buffer) {
			let events_count = bytes_read / mem::size_of::<input_event>();
			let events = unsafe {
				mem::transmute::<[u8; KeyboardMapper::BUF_SIZE], [input_event; KeyboardMapper::MAX_EVS]>(self.raw_buffer)
			};

			for i in 0..events_count {
				let x = events[i];
				if events[i].kind == EV_KEY as u16 {
					let ev = util::map_status_from_linux(events[i].value);
					let result = self.kbct.map_event(KbctEvent { code: events[i].code as i32, ev_type: ev });
					for x in result {
						let value = util::map_status_from_kbct(x.ev_type);
						self.device.write(EV_KEY, x.code, value)?;
					}
				} else {
					self.device.write(x.kind as i32, x.code as i32, x.value)?;
				}
			}
			Ok(ObserverResult::Nothing)
		} else {
			Ok(ObserverResult::Unsubcribe)
		}
	}

	fn get_source_fd(&self) -> SourceFd {
		SourceFd(&self.raw_fd)
	}
}

struct DeviceManager {
	inotify: Inotify,
	conf: KbctRootConf,
	captured_kb_paths: HashSet<String>,
	raw_fd: RawFd,
}

impl DeviceManager {
	pub const SYNTHETIC_EV_FILE: &'static str = "__kbct_synthetic_event";

	fn new(conf: KbctRootConf) -> Result<Box<DeviceManager>> {
		let mut inotify = inotify::Inotify::init()
			.expect("Error while initializing inotify instance");
		let raw_fd = inotify.as_raw_fd();
		let captured_kb_paths = hashset! {};

		inotify
			.add_watch(
				"/dev/input",
				inotify::WatchMask::CREATE | inotify::WatchMask::DELETE,
			).expect("Failed to add file watch on /dev/input/*");

		Ok(Box::new(DeviceManager { inotify, conf, raw_fd, captured_kb_paths }))
	}

	fn force_try_capture_device() {
		thread::spawn(move || {
			thread::sleep(time::Duration::from_millis(100));
			let path = format!("/dev/input/{}", DeviceManager::SYNTHETIC_EV_FILE);
			File::create(&path).unwrap();
			fs::remove_file(&path).unwrap();
		});
	}

	fn update_captured_kbs(&mut self) -> Result<Vec<Box<dyn EventObserver>>> {
		let available_kbs = util::get_all_uinput_device_paths()?;

		let available_kb_paths: HashMap<&String, &String> = available_kbs.iter()
			.map(|x| (x.1, x.0)).collect();

		self.captured_kb_paths.retain(|x| {
			if available_kb_paths.contains_key(x) {
				info!("Ejected device name={:?} path={:?}", available_kb_paths.get(x).unwrap(), x);
				true
			} else {
				false
			}
		});


		let mut ans: Vec<Box<dyn EventObserver>> = vec![];

		for (kb_alias, conf) in self.conf.modifications.iter() {
			if let Some(kb_name) = self.conf.keyboards.get(kb_alias) {
				if let Some(kb_path) = available_kbs.get(kb_name) {
					if !self.captured_kb_paths.contains(kb_path) {
						let kb_new_name = format!("{}-{}", "Kbct", kb_name);
						let file = util::open_readable_uinput_device(kb_path, true)?;
						let raw_fd = file.as_raw_fd();
						let device = util::create_writable_uinput_device(&kb_new_name)?;
						let raw_buffer = [0; KeyboardMapper::BUF_SIZE];
						let kbct = Kbct::new(conf.clone(), util::linux_keyname_mapper)?;

						let mapper = Box::new(
							KeyboardMapper { file, device, raw_buffer, kbct, raw_fd });

						ans.push(mapper);
						self.captured_kb_paths.insert(kb_path.clone());

						info!("Capturing device path={} name={:?} mapped_name={:?}",
									kb_path, kb_name, kb_new_name)
					}
				}
			}
		}
		Ok(ans)
	}
}

impl EventObserver for DeviceManager {
	fn on_event(&mut self, _: &Event) -> Result<ObserverResult> {
		use inotify::EventMask;
		let mut buffer = [0; 1024];
		let regex: Regex = Regex::new("^(event\\d+)|(__kbct_synthetic_event)$")?;
		let events = self.inotify.read_events_blocking(&mut buffer)
			.expect("Error while reading events");

		let has_updates =
			events.into_iter()
				.find(|event| regex.is_match(event.name.unwrap().to_str().unwrap()) &&
					!event.mask.contains(EventMask::ISDIR) &&
					(event.mask.contains(EventMask::CREATE) ||
						event.mask.contains(EventMask::DELETE)))
				.is_some();

		if has_updates {
			Ok(ObserverResult::SubscribeNew(
				DeviceManager::update_captured_kbs(self)
					.expect("Could not capture keyboard")))
		} else {
			Ok(ObserverResult::Nothing)
		}
	}

	fn get_source_fd(&self) -> SourceFd {
		SourceFd(&self.raw_fd)
	}
}


fn start_mapper(config_file: String) -> Result<()> {
	let config = KbctRootConf::parse(
		std::fs::read_to_string(config_file.as_str())
			.expect(&format!("Could not open file {}", config_file))
	).expect("Could not parse the configuration yaml file");

	let mut evloop = EventLoop::new()?;

	evloop.register_observer(SignalReceiver::new()?)?;
	evloop.register_observer(DeviceManager::new(config)?)?;

	DeviceManager::force_try_capture_device();

	info!("Starting kbct event loop, pid={}", process::id());
	evloop.run()?;

	Ok(())
}

fn show_device_names() -> Result<()> {
	for (name, path) in util::get_all_uinput_device_paths()? {
		println!("{}\t{:?}", path, name)
	}
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
	#[clap()]
	ListDevices(ListDevices),
}

#[derive(Clap)]
struct CliTestReplay {
	#[clap(short, long)]
	testcase: String,
}

#[derive(Clap)]
struct CliRemap {
	#[clap(short, long)]
	config: String,
}

#[derive(Clap)]
struct ListDevices {}

fn main() -> Result<()> {
	pretty_env_logger::init();
	let root_opts: CliRoot = CliRoot::parse();
	use SubCommand::*;
	match root_opts.subcmd {
		TestReplay(args) => {
			util::integration_test::replay(args.testcase)?;
		}
		Remap(args) => {
			start_mapper(args.config)?;
		}
		ListDevices(_) => {
			show_device_names()?;
		}
	}
	Ok(())
}

mod nio;
mod util;
