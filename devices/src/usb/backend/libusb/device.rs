// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std;

use usb::libusb::bindings::*;
use usb::libusb::config_descriptor::*;
use usb::libusb::error::*;
use usb::libusb::libusb_context::*;
use usb::libusb::device_handle::*;
use usb::libusb::device_descriptor::*;
use usb::libusb::types::*;


pub struct Device<'a> {
    _context: std::marker::PhantomData<&'a LibUSBContext>,
    device: *mut libusb_device,
}

impl<'a> Drop for Device<'a> {
    fn drop(&mut self) {
        unsafe {
            libusb_unref_device(self.device);
        }
    }
}

impl<'a> Device<'a> {
    pub fn new(_c: &'a LibUSBContext, device: *mut libusb_device) -> Device<'a> {
        unsafe {
            libusb_ref_device(device);
        }
        Device {
            _context: std::marker::PhantomData,
            device: device,
        }
    }

    pub fn get_device_descriptor(&self) -> Result<DeviceDescriptor> {
        let mut descriptor: libusb_device_descriptor = unsafe { std::mem::uninitialized() };
        call_libusb_fn!(libusb_get_device_descriptor(self.device, &mut descriptor));
        Ok(DeviceDescriptor::new(descriptor))
    }

    pub fn get_config_descriptor(&self, idx: u8) -> Result<ConfigDescriptor> {
        let mut descriptor: *mut libusb_config_descriptor = std::ptr::null_mut();
        call_libusb_fn!(libusb_get_config_descriptor(self.device, idx, &mut descriptor));
        Ok(ConfigDescriptor::new(descriptor))
    }

    pub fn get_bus_number(&self) -> u8 {
        unsafe {
            libusb_get_bus_number(self.device)
        }
    }

    pub fn get_address(&self) -> u8 {
        unsafe {
            libusb_get_device_address(self.device)
        }
    }

    pub fn get_speed(&self) -> Speed {
        let speed = unsafe {
            libusb_get_device_speed(self.device)
        };
        Speed::new(speed as u32)
    }

    pub fn open(&self) -> Result<DeviceHandle> {
        let mut handle: *mut libusb_device_handle = std::ptr::null_mut();
        call_libusb_fn!(libusb_open(self.device, &mut handle));
        Ok(DeviceHandle::new(self._context, handle))
    }
}

