// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std;
use std::os::raw::c_int;
use std::marker::PhantomData;

use usb::libusb::bindings::*;
use usb::libusb::error::*;
use usb::libusb::libusb_context::*;

pub struct DeviceHandle<'a> {
    _context: PhantomData<&'a LibUsbContext>,
    handle: *mut libusb_device_handle,
}

impl <'a> Drop for DeviceHandle<'a> {
    fn drop(&mut self) {
        unsafe {
            libusb_close(self.handle);
        }
    }
}

impl<'a> DeviceHandle<'a> {
    pub fn new(c: PhantomData<&'a LibUsbContext>, handle: *mut libusb_device_handle) -> DeviceHandle<'a> {
        DeviceHandle {
            _context: c,
            handle: handle,
        }
    }

    pub fn get_active_configuration(&self) -> Result<i32> {
        let mut config: c_int = 0;
        call_libusb_fn!(libusb_get_configuration(self.handle, &mut config));
        Ok(config as i32)
    }

    pub fn set_active_configuration(&mut self, config: i32) -> Result<()> {
        call_libusb_fn!(libusb_set_configuration(self.handle, config as c_int));
        Ok(())
    }

    pub fn claim_interface(&self, interface_number: i32) -> Result<()> {
        call_libusb_fn!(libusb_claim_interface(self.handle, interface_number));
        Ok(())
    }

    pub fn release_interface(&self, interface_number: i32) -> Result<()> {
        call_libusb_fn!(libusb_release_interface(self.handle, interface_number));
        Ok(())
    }

    pub fn reset_device(&self) -> Result<()> {
        call_libusb_fn!(libusb_reset_device(self.handle));
        Ok(())
    }

    pub fn kernel_driver_active(&self, interface_number: i32) -> Result<bool> {
        let v = call_libusb_fn!(libusb_kernel_driver_active(self.handler, interface_number));
        Ok(v != 0)
    }

    pub fn detach_kernel_driver(&self, interface_number: i32) -> Result<()> {
         call_libusb_fn!(libusb_detach_kernel_driver(self.handler, interface_number));
         Ok(())
    }

    pub fn attach_kernel_driver(&self, interface_number: i32) -> Result<()> {
        call_libusb_fn!(libusb_attach_kernel_driver(self.handler, interface_number));
        Ok(())
    }
}

