use mio::{Events, Poll, Token, Interest};
use std::collections::HashMap;
use std::io;
use std::io::{ErrorKind, Error};
use std::os::unix::io::RawFd;
use mio::unix::SourceFd;
use kbct::{Result, KbctError};
use mio::event::{Event, Source};
use kbct::KbctError::IOError;
use log::info;

const EVENTS_CAPACITY: usize = 1024;

pub struct EventLoop {
	events: Events,
	registrar: EventLoopRegistrar,
}

pub struct EventLoopRegistrar {
	poll: Poll,
	running: bool,
	handlers: HashMap<Token, Box<dyn EventObserver>>,
	last_token: usize,
}

pub enum ObserverResult {
	Nothing,
	Unsubcribe,
	Terminate {
		status: i32
	},
	SubscribeNew(Vec<Box<dyn EventObserver>>),
}

impl EventLoop {
	pub(crate) fn new() -> Result<EventLoop> {
		Ok(EventLoop {
			events: Events::with_capacity(EVENTS_CAPACITY),
			registrar: EventLoopRegistrar {
				poll: Poll::new()?,
				running: true,
				handlers: HashMap::new(),
				last_token: 0,
			},
		})
	}

	pub(crate) fn run(&mut self) -> Result<()> {
		while self.registrar.running {
			self.registrar.poll.poll(&mut self.events, None)?;
			for ev in self.events.iter() {
				let handler = self.registrar.handlers.get_mut(&ev.token()).unwrap();
				match handler.on_event(ev)? {
					ObserverResult::Nothing => {}
					ObserverResult::Unsubcribe => {
						handler.get_source_fd().deregister(self.registrar.poll.registry())?;
						self.registrar.handlers.remove(&ev.token());
					}
					ObserverResult::Terminate { status: _status } => {
						self.registrar.running = false;
					}
					ObserverResult::SubscribeNew(observers) => {
						for obs in observers {
							EventLoop::do_register_observer(&mut self.registrar, obs)?;
						}
					}
				}
			}
		}
		Ok(())
	}

	pub fn register_observer(&mut self, obs: Box<dyn EventObserver>) -> Result<()> {
		Ok(EventLoop::do_register_observer(&mut self.registrar, obs)?)
	}


	fn do_register_observer(reg: &mut EventLoopRegistrar, obs: Box<dyn EventObserver>) -> Result<()> {
		let mut fd = obs.get_source_fd();
		let token = Token(reg.last_token);
		reg.last_token += 1;
		reg.poll.registry().register(&mut fd, token, Interest::READABLE)?;
		assert!(reg.handlers.get(&token).is_none(), "Token handler is already set");
		reg.handlers.insert(token, obs);
		Ok(())
	}
}

pub trait EventObserver {
	fn on_event(&mut self, _: &Event) -> Result<ObserverResult>;
	fn get_source_fd(&self) -> SourceFd;
}
