use std::{thread, time};

use uinput::Device;
use uinput::device::Builder;

fn main() {
	fn find_device() {
		let device = Builder::open("/dev/input/event3").unwrap().create().unwrap();

	}

	fn open_uinput_device() -> Device {
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
		builder.create().unwrap()
	}

	let device = open_uinput_device();
	thread::sleep(time::Duration::from_millis(100000));
}