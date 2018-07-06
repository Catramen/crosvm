// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::collections::BTreeMap;
use std::mem::drop;
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::{Arc, Weak};
use std::thread;
use sync::Mutex;
use sys_util::{EpollContext, EpollEvents, EventFd, PollToken, WatchingEvents};

/// Fd is a wrapper of RawFd. It implements AsRawFd trait and PollToken trait for RawFd.
/// It does not own the fd, thus won't close the fd when dropped.
pub struct Fd(pub RawFd);
impl AsRawFd for Fd {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl PollToken for Fd {
    fn as_raw_token(&self) -> u64 {
        self.0 as u64
    }

    fn from_raw_token(data: u64) -> Self {
        Fd(data as RawFd)
    }
}

/// EpollEventLoop is an event loop blocked on a set of fds. When a monitered events is triggered,
/// event loop will invoke the mapped handler.
pub struct EventLoop {
    poll_ctx: Arc<EpollContext<Fd>>,
    handlers: Arc<Mutex<BTreeMap<RawFd, Weak<EventHandler>>>>,
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

impl EventLoop {
    /// Start an event loop.
    pub fn start() -> (EventLoop, thread::JoinHandle<()>) {
        let (self_stop_evt, stop_evt) = match EventFd::new().and_then(|e| Ok((e.try_clone()?, e))) {
            Ok(v) => v,
            Err(_e) => panic!("failed creating cmd EventFd pair"),
        };

        let fd_callbacks: Arc<Mutex<BTreeMap<RawFd, Weak<EventHandler>>>> =
            Arc::new(Mutex::new(BTreeMap::new()));
        let poll_ctx: EpollContext<Fd> =
            match EpollContext::new().and_then(|pc| pc.add(&stop_evt, Fd(stop_evt.as_raw_fd())).and(Ok(pc))) {
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
            let event_loop = EpollEvents::new();
            loop {
                let events = poll_ctx.wait(&event_loop).expect("Unable to poll");
                for event in events.iter() {
                    if event.token().as_raw_fd() == stop_evt.as_raw_fd() {
                        return;
                    } else {
                        let fd = event.token().as_raw_fd();
                        let mut locked = fd_callbacks.lock();
                        let weak_handler = match locked.get(&fd) {
                            Some(cb) => cb.clone(),
                            None => {
                                warn!("callback for fd {} already removed", fd);
                                continue;
                            }
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
                                if locked.remove(&fd).is_none() {
                                    error!("fail to remove handler for file descriptor {}", fd);
                                }
                            }
                        };
                    }
                }
            }
        });

        (event_loop, handle)
    }

    /// Add a new event to event loop. The event handler will be invoked when `event` happens on
    /// `fd`.
    ///
    /// If the same `fd` is added multiple times, the old handler will be replaced.
    /// EventLoop will not keep `handler` alive, if handler is dropped when `event` is triggered, the
    /// event will be removed.
    pub fn add_event(&self, fd: &AsRawFd, events: WatchingEvents, handler: Weak<EventHandler>) {
        self.handlers.lock().insert(fd.as_raw_fd(), handler);
        // This might fail due to epoll syscall. Check epoll_ctl(2).
        self.poll_ctx
            .add_fd_with_events(fd, events, Fd(fd.as_raw_fd()))
            .expect("fail to add event to epoll context");
    }

    /// Removes event for this `fd`.
    ///
    /// EventLoop does not guarantee all events for `fd` is handled.
    pub fn remove_event_for_fd(&self, fd: &AsRawFd) {
        // This might fail due to epoll syscall. Check epoll_ctl(2).
        self.poll_ctx
            .delete(fd)
            .expect("fail to delete event from epoll context");
        self.handlers.lock().remove(&fd.as_raw_fd());
    }

    /// Stops this event loop asynchronously. Triggered events might not be handled.
    pub fn stop(&self) {
        match self.stop_evt.write(1) {
            Ok(_) => {
                debug!("event loop stopped");
            }
            Err(_) => {
                debug!("fail to send event loop stop event, it might already stopped");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
    use std::sync::{Arc, Condvar, Mutex};
    use sys_util::EventFd;

    struct EventLoopTestHandler {
        val: Mutex<u8>,
        cvar: Condvar,
    }

    impl EventHandler for EventLoopTestHandler {
        fn on_event(&self, fd: RawFd) {
            let _ = unsafe { EventFd::from_raw_fd(fd).read() };
            *self.val.lock().unwrap() += 1;
            self.cvar.notify_one();
        }
    }

    #[test]
    fn event_loop_test() {
        let (l, j) = EventLoop::start();
        let (self_evt, evt) = match EventFd::new().and_then(|e| Ok((e.try_clone()?, e))) {
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
        l.add_event(evt, WatchingEvents::empty().set_read(), Arc::downgrade(&t));
        self_evt.write(1).unwrap();
        let _ = h.cvar.wait(h.val.lock().unwrap()).unwrap();
        l.stop();
        j.join().unwrap();
        assert_eq!(*(h.val.lock().unwrap()), 1);
    }
}
