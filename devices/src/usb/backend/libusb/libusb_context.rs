// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std;

use usb::libusb::bindings::*;
use usb::libusb::error::*;
use usb::libusb::device::*;
use usb::libusb::libusb_pollfd::*;

// Wrapper for libusb_context. The libusb libary initialization/deinitialization
// is managed by this context.
// See: http://libusb.sourceforge.net/api-1.0/group__libusb__lib.html
pub struct LibUsbContext {
    context: *mut libusb_context,
}

impl Drop for LibUsbContext {
    fn drop(&mut self) {
        unsafe {
            libusb_exit(self.context);
        }
    }
}

impl LibUsbContext {
    pub fn new() -> Result<LibUsbContext> {
        let mut ctx: *mut libusb_context = std::ptr::null_mut();
        call_libusb_fn!(libusb_init(&mut ctx));
        Ok(LibUsbContext { context: ctx })
    }

    // Device list is allocated by libusb and freed before this function returns.
    // The devices handles are still alive cause libusb_ref_device is called when
    // Device is constructed.
    // See http://libusb.sourceforge.net/api-1.0/group__libusb__dev.html.
    pub fn get_device_list(&self) -> Result<std::vec::Vec<Device>> {
        let mut list: *mut *mut libusb_device = std::ptr::null_mut();
        let n = call_libusb_fn!(libusb_get_device_list(self.context, &mut list));

        let mut vec = Vec::new();
        for i in 0..n {
            unsafe {
                vec.push(Device::new(self, *list.offset(i as isize)));
            }
        }

        unsafe {
            libusb_free_device_list(list, 1);
        }
        Ok(vec)
    }

    pub fn has_capability(&self, cap: u32) -> bool {
        unsafe {
            libusb_has_capability(cap) != 0
        }
    }

    pub fn get_pollfds(&self) -> Result<std::vec::Vec<libusb_pollfd>> {
        let mut vec = Vec::new();
        let mut list: *mut *mut libusb_pollfd = unsafe {
            libusb_get_pollfds(self.context)
        }
        unsafe {
            let mut idx = 0;
            while !list.offset(idx).is_null() {
                vec.push(**list.offset(idx));
                idx ++;
            }
            libusb_free_pollfds(list);
        }
        vec
    }

    pub fn set_pollfd_notifiers<T: LibUsbPollfdChangeHandler>(&self,
                                                              handler: T) -> Box<PollfdHandlerKeeper> {
        PollfdHandlerKeeper::new(self, handler)
    }

}

