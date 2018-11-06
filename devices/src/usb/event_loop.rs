// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use sys_util::{EventFd, PollContext, WatchingEvents};
use std::collections::HashMap;
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::{Arc, Weak, Mutex};
use std::mem::drop;
use std::thread;

/// EpollEventLoop is an event loop blocked on a set of fds. When a monitered events is triggered,
/// event loop will invoke the mapped handler.
pub struct EventLoop {
    poll_ctx: Arc<PollContext<u32>>,
    handlers: Arc<Mutex<HashMap<RawFd, Weak<EventHandler>>>>,
    stop_evt: EventFd,
}

impl Clone for EventLoop {
    fn clone(&self) -> EventLoop {
        EventLoop {
            poll_ctx: self.poll_ctx.clone(),
            handlers: self.handlers.clone(),
            stop_evt: self.stop_evt.try_clone().unwrap(),
        }
    }
}

/// Interface for event handler.
pub trait EventHandler: Send + Sync {
    fn on_event(&self, fd: RawFd);
}

struct Fd(RawFd);
impl AsRawFd for Fd {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl EventLoop {
    /// Start an event loop.
    pub fn start() -> (EventLoop, thread::JoinHandle<()>) {
        let (self_stop_evt, stop_evt) = match EventFd::new().and_then(|e| Ok((e.try_clone()?, e))) {
            Ok(v) => v,
            Err(_e) => panic!("failed creating cmd EventFd pair"),
        };

        let mut fd_callbacks: Arc<Mutex<HashMap<RawFd, Weak<EventHandler>>>>
            = Arc::new(Mutex::new(HashMap::new()));
        let poll_ctx: PollContext<u32> = match PollContext::new()
            .and_then(|pc| pc.add(&stop_evt, stop_evt.as_raw_fd() as u32).and(Ok(pc)))
            {
                Ok(pc) => pc,
                Err(_e) => panic!("failed creating PollContext"),
            };
        let poll_ctx = Arc::new(poll_ctx);
        let event_loop = EventLoop {
            poll_ctx: poll_ctx.clone(),
            handlers: fd_callbacks.clone(),
            stop_evt: self_stop_evt,
        };

        let handle = thread::spawn(move || {
            loop {
                let events = poll_ctx.wait().expect("Unable to poll");
                for event in events.iter() {
                    if event.token() == stop_evt.as_raw_fd() as u32 {
                        return;
                    } else {
                        let fd = event.token() as RawFd;
                        let mut locked = fd_callbacks.lock().unwrap();
                        let weak_handler = match locked.get(&fd) {
                            Some(cb) => cb.clone(),
                            None => {
                                warn!("callback for fd {} already removed", fd);
                                continue;
                            },
                        };
                        match weak_handler.upgrade() {
                            Some(handler) => {
                                // Drop lock before triggering the event.
                                drop(locked);
                                handler.on_event(fd);
                            }
                            // If the handler is already gone, we remove the fd.
                            None => {
                                let _ = poll_ctx.delete(&Fd(fd));
                                locked.remove(&fd).unwrap();
                            },
                        };
                    }
                }
            }
        });

        (event_loop, handle)
    }

    /// Add event to event loop.
    pub fn add_event(&self, fd: RawFd, events: WatchingEvents, handler: Weak<EventHandler>) {
        self.poll_ctx.add_fd_with_events(&Fd(fd), events, fd as u32).unwrap();
        self.handlers.lock().unwrap().insert(fd, handler);
    }

    /// Removes event for this RawFd.
    pub fn remove_event_for_fd(&self, fd: RawFd) {
        self.poll_ctx.delete(&Fd(fd)).unwrap();
        self.handlers.lock().unwrap().remove(&fd);
    }

    /// Stops this event loop asynchronously. Triggered events might not be handled.
    pub fn stop(self) {
        self.stop_evt.write(1).unwrap();
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

