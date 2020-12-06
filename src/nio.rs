use mio::{Events, Poll, Token, Interest};
use std::collections::HashMap;
use std::io;
use std::io::{ErrorKind, Error};
use mio::unix::SourceFd;
use kbct::{Result, KbctError};
use mio::event::Event;
use kbct::KbctError::IOError;

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
	SubscribeNew(Box<dyn EventObserver>),
}

impl EventLoop {
	fn run(&mut self) -> Result<()> {
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

	pub fn register_observer(&mut self, fd: i32, token: Token, obs: Box<dyn EventObserver>) -> Result<()> {
		self.poll.registry().register(&mut SourceFd(&fd), token, Interest::READABLE)?;
		if self.handlers.contains_key(&token) {
			Err(KbctError::Error("Already exists".to_string()))
		} else {
			self.handlers.insert(token, obs);
			Ok(())
		}
	}
}

pub trait EventObserver {
	fn on_event(&mut self, _: &Event) -> Result<ObserverResult>;
}
