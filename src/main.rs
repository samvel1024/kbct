extern crate chrono;
#[macro_use]
extern crate log;
#[macro_use]
extern crate maplit;
extern crate pretty_env_logger;
extern crate uinput;
extern crate uinput_sys;

use std::{fs, fs::File, process, thread, time};
use std::collections::{HashMap, HashSet};
use std::os::unix::io::{AsRawFd, RawFd};

use clap::Clap;
use inotify::Inotify;
use log::LevelFilter;
use mio::event::Event;
use mio::unix::SourceFd;
use nix::sys::signal::SigSet;
use nix::sys::signalfd::SignalFd;
use regex::Regex;
use uinput::Device;
use uinput_sys::*;

use kbct::*;
use nio::*;

struct SignalReceiver {
	#[allow(dead_code)]
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
			&mask,
			nix::sys::signalfd::SfdFlags::SFD_NONBLOCK,
		)
			.unwrap();
		let fd = sfd.as_raw_fd();
		Ok(Box::new(SignalReceiver {
			signal_fd: (sfd),
			raw_fd: fd,
		}))
	}
}

impl EventObserver for SignalReceiver {
	fn on_event(&mut self, _: &Event) -> Result<ObserverResult> {
		info!("Received signal, stopping");
		Ok(ObserverResult::Terminate { status: 0 })
	}

	fn get_source_fd(&self) -> SourceFd {
		SourceFd(&self.raw_fd)
	}
}

struct KeyboardMapper {
	file: File,
	device: Device,
	raw_buffer: util::KeyBuffer,
	kbct: Kbct,
	raw_fd: RawFd,
}

impl EventObserver for KeyboardMapper {
	fn on_event(&mut self, _: &Event) -> Result<ObserverResult> {
		if let Ok(uinput_events) = util::read_key_events(&mut self.file, &mut self.raw_buffer) {
			for ev in uinput_events {
				if let Some(kbct_ev) = util::kbct_from_uinput_event(&ev) {
					let result = self.kbct.map_event(kbct_ev);
					debug!("{}", util::KeyMapEvent::from_kbct_event(kbct_ev, &result));
					for x in result {
						let value = util::map_status_from_kbct(x.ev_type);
						self.device.write(EV_KEY, x.code, value)?;
					}
				} else {
					self.device
						.write(ev.kind as i32, ev.code as i32, ev.value)?;
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
		let mut inotify =
			inotify::Inotify::init().expect("Error while initializing inotify instance");
		let raw_fd = inotify.as_raw_fd();
		let captured_kb_paths = hashset! {};

		inotify
			.add_watch(
				"/dev/input",
				inotify::WatchMask::CREATE | inotify::WatchMask::DELETE,
			)
			.expect("Failed to add file watch on /dev/input/*");

		Ok(Box::new(DeviceManager {
			inotify,
			conf,
			raw_fd,
			captured_kb_paths,
		}))
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
		let available_kb_names = util::get_all_uinput_device_names_to_paths()?;

		let available_kb_paths: HashMap<&String, &String> =
			available_kb_names.iter().map(|x| (x.1, x.0)).collect();

		self.captured_kb_paths.retain(|x| {
			if available_kb_paths.contains_key(x) {
				true
			} else {
				info!("Ejected device path={:?}", x);
				false
			}
		});

		let mut ans: Vec<Box<dyn EventObserver>> = vec![];

		for conf in self.conf.iter() {
			for kb_name in conf.keyboards() {
				if let Some(kb_path) = available_kb_names.get(kb_name) {
					if !self.captured_kb_paths.contains(kb_path) {
						let kb_new_name = format!("{}-{}", "Kbct", kb_name);
						let file = util::open_readable_uinput_device(kb_path, true)?;
						let raw_fd = file.as_raw_fd();
						let device = util::create_writable_uinput_device(&kb_new_name)?;
						let raw_buffer: util::KeyBuffer = [0; util::BUF_SIZE];
						let kbct = Kbct::new(conf.clone(), util::linux_keyname_mapper)?;

						let mapper = Box::new(KeyboardMapper {
							file,
							device,
							raw_buffer,
							kbct,
							raw_fd,
						});

						ans.push(mapper);
						self.captured_kb_paths.insert(kb_path.clone());

						info!(
							"Capturing device path={} name={:?} mapped_name={:?}",
							kb_path, kb_name, kb_new_name
						)
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
		let events = self
			.inotify
			.read_events_blocking(&mut buffer)
			.expect("Error while reading events");

		let has_updates = events
			.into_iter()
			.find(|event| {
				regex.is_match(event.name.unwrap().to_str().unwrap())
					&& !event.mask.contains(EventMask::ISDIR)
					&& (event.mask.contains(EventMask::CREATE)
					|| event.mask.contains(EventMask::DELETE))
			})
			.is_some();

		if has_updates {
			self.update_captured_kbs()
				.map(|observer| ObserverResult::SubscribeNew(observer))
				.or(Ok(ObserverResult::Nothing))
		} else {
			Ok(ObserverResult::Nothing)
		}
	}

	fn get_source_fd(&self) -> SourceFd {
		SourceFd(&self.raw_fd)
	}
}

struct KeyLogger {
	device_file: File,
	raw_fd: RawFd,
	raw_buffer: util::KeyBuffer,
}

impl KeyLogger {
	fn new(device: String) -> Result<Box<KeyLogger>> {
		let device_file = util::open_readable_uinput_device(&device, false)
			.expect(format!("Could not open readable device {}", device).as_str());
		let raw_fd = device_file.as_raw_fd();
		Ok(Box::new(KeyLogger {
			device_file,
			raw_fd,
			raw_buffer: [0; util::BUF_SIZE],
		}))
	}
}

impl EventObserver for KeyLogger {
	fn on_event(&mut self, _: &Event) -> Result<ObserverResult> {
		if let Ok(events) = util::read_key_events(&mut self.device_file, &mut self.raw_buffer) {
			for ev in events {
				if let Some(kbct_event) = util::kbct_from_uinput_event(&ev) {
					println!(
						"{}",
						format!(
							"{} {:?}",
							util::keycodes::code_to_name(kbct_event.code),
							kbct_event.ev_type
						)
							.to_lowercase()
					)
				}
			}
			Ok(ObserverResult::Nothing)
		} else {
			Ok(ObserverResult::Terminate { status: 0 })
		}
	}

	fn get_source_fd(&self) -> SourceFd {
		SourceFd(&self.raw_fd)
	}
}

fn start_mapper_from_file_conf(config_file: String) -> Result<()> {
	let config = serde_yaml::from_str(
		&*std::fs::read_to_string(config_file.as_str())
			.expect(&format!("Could not open file {}", config_file)))
		.expect("Could not parse the configuration yaml file");
	start_mapper(config)
}

fn start_mapper(config: KbctRootConf) -> Result<()> {
	let mut evloop = EventLoop::new()?;

	evloop.register_observer(SignalReceiver::new()?)?;
	evloop.register_observer(DeviceManager::new(config)?)?;

	DeviceManager::force_try_capture_device();

	info!("Starting kbct event loop, pid={}", process::id());
	evloop.run()?;

	Ok(())
}

fn show_device_names() -> Result<()> {
	for (name, path) in util::get_all_uinput_device_names_to_paths()? {
		println!("{}\t{:?}", path, name)
	}
	Ok(())
}

fn log_keys(device: String) -> Result<()> {
	let mut evloop = EventLoop::new()?;

	evloop.register_observer(SignalReceiver::new()?)?;
	evloop.register_observer(KeyLogger::new(device)?)?;

	evloop.run()?;
	Ok(())
}

#[derive(Clap)]
struct CliRoot {
	#[clap(subcommand)]
	subcmd: SubCommand,
	#[clap(short, long)]
	debug_log: bool,
}

#[derive(Clap)]
enum SubCommand {
	#[clap()]
	TestReplay(CliTestReplay),
	#[clap()]
	Remap(CliRemap),
	#[clap()]
	ListDevices(ListDevices),
	#[clap()]
	LogKeys(LogKeys),
}

#[derive(Clap)]
struct CliTestReplay {
	#[clap(short, long)]
	testcase: String,
	#[clap(short, long, default_value="DummyDevice")]
	device_name: String
}

#[derive(Clap)]
struct CliRemap {
	#[clap(short, long)]
	config: String,
}

#[derive(Clap)]
struct LogKeys {
	#[clap(short, long)]
	device_path: String,
}

#[derive(Clap)]
struct ListDevices {}

fn main() -> Result<()> {
	let root_opts: CliRoot = CliRoot::parse();
	pretty_env_logger::formatted_builder()
		.filter_level(if root_opts.debug_log {
			LevelFilter::Debug
		} else {
			LevelFilter::Info
		})
		.init();
	use SubCommand::*;
	match root_opts.subcmd {
		TestReplay(args) => {
			util::integration_test::replay(args.testcase, args.device_name)?;
		}
		Remap(args) => {
			start_mapper_from_file_conf(args.config)?;
		}
		ListDevices(_) => {
			show_device_names()?;
		}
		LogKeys(args) => {
			log_keys(args.device_path)?;
		}
	}
	Ok(())
}

mod nio;
mod util;
