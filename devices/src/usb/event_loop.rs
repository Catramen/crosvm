// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use sys_util::{EventFd, PollContext, WatchingEvents};
use std::collections::HashMap;
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::{mpsc, Weak};
use std::thread;

/// EpollEventLoop is an event loop blocked on a set of fds. When a monitered events is triggered,
/// event loop will invoke the mapped handler.
pub struct EventLoop {
    cmd_tx: mpsc::Sender<EpollThreadEvents>,
    cmd_evt: EventFd,
}

/// Interface for event handler.
pub trait EventHandler {
    fn on_event(&self, fd: RawFd);
}

enum EpollThreadEvents {
    Stop,
    // Add pollfd with fd, events and handler.
    AddPollFd(RawFd, WatchingEvents, Weak<EventHandler>),
    DeleteFd(RawFd),
}

unsafe impl Send for EpollThreadEvents {}
unsafe impl Sync for EpollThreadEvents {}

struct Fd(RawFd);
impl AsRawFd for Fd {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl Clone for EventLoop {
    fn clone(&self) -> EventLoop {
        EventLoop {
            cmd_tx: self.cmd_tx.clone(),
            cmd_evt: self.cmd_evt.try_clone().unwrap(),
        }
    }
}
impl EventLoop {
    /// Start an event loop.
    pub fn start() -> (EventLoop, thread::JoinHandle<()>) {
        let (sender, receiver) = mpsc::channel::<EpollThreadEvents>();
        let (self_cmd_evt, cmd_evt) = match EventFd::new().and_then(|e| Ok((e.try_clone()?, e))) {
            Ok(v) => v,
            Err(_e) => panic!("failed creating cmd EventFd pair"),
        };
        let handle = thread::spawn(move || {
            let mut fd_callbacks: HashMap<RawFd, Weak<EventHandler>> = HashMap::new();
            let poll_ctx: PollContext<u32> = match PollContext::new()
                .and_then(|pc| pc.add(&cmd_evt, cmd_evt.as_raw_fd() as u32).and(Ok(pc)))
            {
                Ok(pc) => pc,
                Err(_e) => panic!("failed creating PollContext"),
            };
            loop {
                let events = poll_ctx.wait().expect("Unable to poll");
                for event in events.iter() {
                    if event.token() == cmd_evt.as_raw_fd() as u32 {
                        let cnt = cmd_evt.read().unwrap();
                        for _ in 0..cnt {
                            let ev = receiver.recv().unwrap();
                            match ev {
                                EpollThreadEvents::Stop => return,
                                EpollThreadEvents::AddPollFd(fd, events, handler) => {
                                    poll_ctx.add_fd_with_events(&Fd(fd),
                                                                events,
                                                                fd as u32).unwrap();
                                    fd_callbacks.insert(fd, handler);
                                }
                                EpollThreadEvents::DeleteFd(fd) => {
                                    fd_callbacks.remove(&fd);
                                }
                            }
                        }
                    } else {
                        let fd = event.token() as RawFd;
                        match fd_callbacks.get(&fd).unwrap().upgrade() {
                            Some(handler) => handler.on_event(fd),
                            // If the handler is already gone, we remove the fd.
                            None => {
                                poll_ctx.delete(&Fd(fd)).unwrap();
                                fd_callbacks.remove(&fd).unwrap();
                            },
                        };
                    }
                }
            }
        });

        (EventLoop {
            cmd_tx: sender,
            cmd_evt: self_cmd_evt,
        },
        handle)
    }

    /// Add event to event loop.
    pub fn add_event(&self, fd: RawFd, events: WatchingEvents, handler: Weak<EventHandler>) {
        self.cmd_tx
            .send(EpollThreadEvents::AddPollFd(fd, events, handler)).unwrap();
        self.cmd_evt.write(1).unwrap();
    }

    /// Removes event for this RawFd.
    pub fn remove_event_for_fd(&self, fd: RawFd) {
        /// Simply do nothing if the event loop is stopped.
        let _ = self.cmd_tx.send(EpollThreadEvents::DeleteFd(fd));
        let _ = self.cmd_evt.write(1);
    }

    /// Stops this event loop asynchronously. Triggered events might not be handled.
    pub fn stop(self) {
        self.cmd_tx.send(EpollThreadEvents::Stop).unwrap();
        self.cmd_evt.write(1).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sys_util::EventFd;
    use std::sync::{Arc, Condvar, Mutex};
    use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

    struct EventLoopTestHandler {
        val: Mutex<u8>,
        cvar: Condvar,
    }

    impl EventHandler for EventLoopTestHandler {
        fn on_event(&self, fd: RawFd) {
            let _ = unsafe {EventFd::from_raw_fd(fd).read()};
            *self.val.lock().unwrap() += 1;
            self.cvar.notify_one();
        }
    }

    #[test]
    fn event_loop_test() {
        let (l,j) = EventLoop::start();
        let (self_evt, evt) =
            match EventFd::new().and_then(|e| Ok((e.try_clone()?, e))) {
                Ok(v) => v,
                Err(e) => {
                    error!("failed creating EventFd pair: {:?}", e);
                    return;
                }
            };
        let h = Arc::new(EventLoopTestHandler {
            val: Mutex::new(0),
            cvar: Condvar::new(),
        });
        let t: Arc<EventHandler> = h.clone();
        l.add_event(EventFd::as_raw_fd(&evt),
                    WatchingEvents::empty().set_read(),
                    Arc::downgrade(&t));
        self_evt.write(1);
        h.cvar.wait(h.val.lock().unwrap());
        l.stop();
        j.join();
        assert_eq!(*(h.val.lock().unwrap()), 1);
    }
}
