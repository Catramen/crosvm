// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use usb::event_loop::{EventLoop, EventHandler};
use usb_util::libusb_context::{LibUsbContext, LibUsbPollfdChangeHandler};
use std::sync::{Arc, Weak, Mutex};
use std::os::unix::io::RawFd;
use std::os::raw::c_short;
use sys_util::WatchingEvents;
use usb_util::libusb_device::LibUsbDevice;

/// Context wraps libusb context with libusb event handling.
pub struct Context {
    context: LibUsbContext,
    event_loop: Mutex<EventLoop>,
    event_handler: Arc<EventHandler>,
}

impl Context {
    pub fn new(event_loop: EventLoop) -> Context {
        let context = LibUsbContext::new().unwrap();
        let ctx = Context {
            context: context.clone(),
            event_loop: Mutex::new(event_loop),
            event_handler: Arc::new(LibUsbEventHandler{context: context.clone()}),
        };
        ctx.init_event_handler();
        ctx
    }

    fn init_event_handler(&self) {
        for pollfd in self.context.get_pollfd_iter() {
            self.event_loop.lock().unwrap().add_event(pollfd.fd,
                                      WatchingEvents::new(pollfd.events as u32),
                                      Arc::downgrade(&self.event_handler)
                                      );
        }

        self.context.set_pollfd_notifiers(Box::new(
                PollfdChangeHandler {
                    event_loop: self.event_loop.lock().unwrap().clone(),
                    event_handler: Arc::downgrade(&self.event_handler),
                }
                ));

    }

    pub fn get_device(&self, bus: u8, addr: u8) -> Option<LibUsbDevice> {
        for device in self.context.get_device_iter().unwrap() {
            if device.get_bus_number() == bus &&
                device.get_address() == addr {
                    debug!("device found bus {}, addr {}", bus, addr);
                    return Some(device);
                }
        }
        error!("device not found bus {}, addr {}", bus, addr);
        None
    }
}

struct LibUsbEventHandler {
    context: LibUsbContext,
}

impl EventHandler for LibUsbEventHandler {
    fn on_event(&self, _fd: RawFd) {
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

