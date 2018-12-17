// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::os::raw::c_short;
use std::os::unix::io::RawFd;
use std::sync::{Arc, Weak};
use sys_util::WatchingEvents;
use usb::error::{Error, Result};
use usb::event_loop::{EventHandler, EventLoop, Fd};
use usb_util::libusb_context::{LibUsbContext, LibUsbPollfdChangeHandler};
use usb_util::libusb_device::LibUsbDevice;

/// Context wraps libusb context with libusb event handling.
pub struct Context {
    context: LibUsbContext,
    event_loop: Arc<EventLoop>,
    event_handler: Arc<EventHandler>,
}

impl Context {
    /// Create a new context.
    pub fn new(event_loop: Arc<EventLoop>) -> Result<Context> {
        let context = LibUsbContext::new().map_err(err_msg!(Error::BadState))?;
        let ctx = Context {
            context: context.clone(),
            event_loop,
            event_handler: Arc::new(LibUsbEventHandler {
                context: context.clone(),
            }),
        };
        ctx.init_event_handler()?;
        Ok(ctx)
    }

    fn init_event_handler(&self) -> Result<()> {
        for pollfd in self.context.get_pollfd_iter() {
            debug!("event loop add event {} events handler", pollfd.fd);
            self.event_loop.add_event(
                &Fd(pollfd.fd),
                WatchingEvents::new(pollfd.events as u32),
                Arc::downgrade(&self.event_handler),
            );
        }

        self.context
            .set_pollfd_notifiers(Box::new(PollfdChangeHandler {
                event_loop: self.event_loop.clone(),
                event_handler: Arc::downgrade(&self.event_handler),
            }));
        Ok(())
    }

    /// Get libusb device with matching bus, addr, vid and pid.
    pub fn get_device(&self, bus: u8, addr: u8, vid: u16, pid: u16) -> Option<LibUsbDevice> {
        let device_iter = match self.context.get_device_iter() {
            Ok(iter) => iter,
            Err(e) => {
                error!("could not get libusb device iterator. error {:?}", e);
                return None;
            }
        };
        for device in device_iter {
            if device.get_bus_number() == bus && device.get_address() == addr {
                if let Ok(descriptor) = device.get_device_descriptor() {
                    if descriptor.idProduct == pid && descriptor.idVendor == vid {
                        return Some(device);
                    }
                }
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
    fn on_event(&self, _fd: RawFd) -> Result<()> {
        self.context.handle_events_nonblock();
        Ok(())
    }
}

struct PollfdChangeHandler {
    event_loop: Arc<EventLoop>,
    event_handler: Weak<EventHandler>,
}

impl LibUsbPollfdChangeHandler for PollfdChangeHandler {
    fn add_poll_fd(&self, fd: RawFd, events: c_short) {
        self.event_loop.add_event(
            &Fd(fd),
            WatchingEvents::new(events as u32),
            self.event_handler.clone(),
        );
    }

    fn remove_poll_fd(&self, fd: RawFd) {
        if let Some(h) = self.event_handler.upgrade() {
            h.on_event(0).unwrap();
        }
        self.event_loop.remove_event_for_fd(&Fd(fd));
    }
}
