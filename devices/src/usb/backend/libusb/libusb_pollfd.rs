// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::boxed::Box;

use usb::libusb::bindings::*;
use std::os::raw::{c_int, c_short};

pub struct PollfdHandlerKeeper<'a> {
    ctx: &'a LibUsbContext,
    handler: Box<LibUsbPollfdChangeHandler>,
}

impl<'a> Drop for PollfdHandlerKeeper<'a> {
    fn drop(&mut self) {
        unsafe {
            libusb_set_pollfd_notifiers(self.ctx, std::ptr::null(),
                                        std::ptr::null(), std::ptr::null);
        }
    }
}

impl<'a> PollfdHandlerKeeper<'a> {
    pub fn new<T: LibUsbPollfdChangeHandler>(ctx: &'a LibUsbContext,
                                             handler: T) -> Box<PollfdHandlerKeeper> {
        let keeper = Box::new(
            PollfdHandlerKeeper {
                ctx: ctx,
                handler: Box::new(handler),
            }
            );
        let raw_keeper = keeper.into(raw);
        unsafe {
            libusb_set_pollfd_notifiers(ctx, PollfdHandlerKeeper::pollfd_added_cb,
                                        PollfdHandlerKeeper::pollfd_removed_cb,
                                        raw_keeper);
        }
        Box::from_raw(raw_keeper)
    }

    pub fn pollfd_added_cb(fd: c_int, events: c_short, keeper: *mut PollfdHandlerKeeper) {
        keeper.handler.add_poll_fd(fd, events);
    }

    pub fn pollfd_removed_cb(fd: c_int, events: c_short, keeper: *mut Pol) {
        keeper.handler.remove_poll_fd(fd, events);
    }

}

pub trait LibUsbPollfdChangeHandler {
    fn add_poll_fd(fd: c_int, events: c_short);
    fn remove_poll_fd(fd: c_int, events: c_short);
}

