// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use usb::libusb::bindings::*;
use usb::libusb::error::*;

pub struct LibUsbTransfer {
    transfer: *mut libusb_transfer,
}

impl Drop for LibUsbTransfer {
    fn drop(&mut self) {
        unsafe {
            libusb_free_transfer(self.transfer);
        }
    }
}

impl LibUsbTransfer {
    pub fn alloc(iso_packets: i32) -> Result<LibUsbTransfer> {
        let mut transfer: *mut libusb_transfer = std::ptr::null_mut();
        unsafe {
            transfer = libusb_alloc_transfer(iso_packets);
        }
        Ok(LibUsbTransfer { transfer: transfer })
    }

    pub fn submit(&self) -> Result<()> {
        call_libusb_fn!(libusb_submit_transfer(self.transfer));
        Ok(())
    }

    pub fn cancel(&self) -> Result<()> {
        call_libusb_fn!(libusb_cancel_transfer(self.transfer));
        Ok(())
    }
}

