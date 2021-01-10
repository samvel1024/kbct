use mio::{Events, Poll, Token, Interest};
use std::collections::HashMap;
use std::io;
use std::io::{ErrorKind, Error};
use mio::unix::SourceFd;
use kbct::{Result, KbctError};
use mio::event::Event;
use kbct::KbctError::IOError;

const EVENTS_CAPACITY: usize = 1024;

pub struct EventLoop {
	events: Events,
	poll: Poll,
	running: bool,
	handlers: HashMap<Token, Box<dyn EventObserver>>,
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
			poll: Poll::new()?,
			events: Events::with_capacity(EVENTS_CAPACITY),
			running: true,
			handlers: HashMap::new(),
		})
	}

	pub(crate) fn run(&mut self) -> Result<()> {
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
					ObserverResult::SubscribeNew(observers) => {
						for obs in observers {
						}
					}
				}
			}
		}
		Ok(())
	}

	pub fn register_observer(&mut self, fd: i32, token: Token, obs: Box<dyn EventObserver>) -> Result<()> {
		self.poll.registry().register(&mut SourceFd(&fd), token, Interest::READABLE)?;
		if self.handlers.contains_key(&token) {
			Err(KbctError::Error("Already exists".to_string()))
		} else {
			assert!(self.handlers.get(&token).is_none(), "Token handler is already set");
			self.handlers.insert(token, obs);
			Ok(())
		}
	}
}

pub trait EventObserver {
	fn on_event(&mut self, _: &Event) -> Result<ObserverResult>;
}
