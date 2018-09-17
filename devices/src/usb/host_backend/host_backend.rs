// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use usb::event_loop::{EventLoop, EventHandler};
use usb_util::libusb_context::{LibUsbContext, LibUsbPollfdChangeHandler};
use std::sync::{Arc, Weak};
use std::os::unix::io::RawFd;
use std::os::raw::c_short;
use sys_util::WatchingEvents;

pub struct HostBackend {
    context: LibUsbContext,
    event_loop: EventLoop,
    event_handler: Arc<EventHandler>,
}

impl HostBackend {
    pub fn new(event_loop: EventLoop) -> HostBackend {
        let context = LibUsbContext::new().unwrap();
        let backend = HostBackend {
            context: context.clone(),
            event_loop: event_loop,
            event_handler: Arc::new(LibUsbEventHandler{context: context.clone()}),
        };
        backend.init_event_handler();
        backend
    }

    fn init_event_handler(&self) {
        for pollfd in self.context.get_pollfd_iter() {
            self.event_loop.add_event(pollfd.fd,
                                      WatchingEvents::new(pollfd.events as u32),
                                      Arc::downgrade(&self.event_handler)
                                      );
        }

        self.context.set_pollfd_notifiers(Box::new(
                PollfdChangeHandler {
                    event_loop: self.event_loop.clone(),
                    event_handler: Arc::downgrade(&self.event_handler),
                }
                ));

    }
}

struct LibUsbEventHandler {
    context: LibUsbContext,
}

impl EventHandler for LibUsbEventHandler {
    fn on_event(&self, fd: RawFd) {
        self.context.handle_events_nonblock();
    }
}

struct PollfdChangeHandler {
    event_loop: EventLoop,
    event_handler: Weak<EventHandler>,
}

impl LibUsbPollfdChangeHandler for PollfdChangeHandler {
    fn add_poll_fd(&self, fd: RawFd, events: c_short) {
        self.event_loop.add_event(fd,
                                  WatchingEvents::new(events as u32),
                                  self.event_handler.clone(),
                                 );
    }

    fn remove_poll_fd(&self, fd: RawFd) {
        self.event_loop.remove_event_for_fd(fd);
    }
}