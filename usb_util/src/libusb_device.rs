// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std;
use std::marker::PhantomData;

use bindings;
use config_descriptor::ConfigDescriptor;
use device_descriptor::DeviceDescriptor;
use device_handle::DeviceHandle;
use error::{Result, Error};
use libusb_context::LibUsbContext;
use types::Speed;

/// LibUsbDevice wraps libusb_device.
pub struct LibUsbDevice<'a> {
    _context: std::marker::PhantomData<&'a LibUsbContext>,
    device: *mut bindings::libusb_device,
}

impl<'a> Drop for LibUsbDevice<'a> {
    fn drop(&mut self) {
        // Safe because 'self.device' is a valid pointer and libusb_ref_device is invoked when
        // 'self' is built.
        unsafe {
            bindings::libusb_unref_device(self.device);
        }
    }
}

impl<'a> LibUsbDevice<'a> {
    /// Create a new LibUsbDevice. 'device' should be a valid pointer to libusb_device.
    pub unsafe fn new(_c: PhantomData<&'a LibUsbContext>, device: *mut bindings::libusb_device)
        -> LibUsbDevice<'a> {
        bindings::libusb_ref_device(device);
        LibUsbDevice {
            _context: _c,
            device: device,
        }
    }

    /// Get device descriptor of this device.
    pub fn get_device_descriptor(&self) -> Result<DeviceDescriptor> {
        // Safe because memory is initialized later.
        let mut descriptor: bindings::libusb_device_descriptor = unsafe { std::mem::uninitialized() };
        // Safe because 'self.device' is valid and '&mut descriptor' is valid.
        handle_libusb_error!(unsafe {
            bindings::libusb_get_device_descriptor(self.device, &mut descriptor)
        });
        Ok(DeviceDescriptor::new(descriptor))
    }

    /// Get config descriptor at index of idx.
    pub fn get_config_descriptor(&self, idx: u8) -> Result<ConfigDescriptor> {
        let mut descriptor: *mut bindings::libusb_config_descriptor = std::ptr::null_mut();
        // Safe because 'self.device' is valid and '&mut descriptor' is valid.
        handle_libusb_error!(unsafe {
            bindings::libusb_get_config_descriptor(
                self.device,
                idx,
                &mut descriptor
                )
        });
        // Safe because descriptor is inited with valid pointer.
        Ok(unsafe {
            ConfigDescriptor::new(descriptor)
        })
    }

    /// Get active config descriptor of this device.
    pub fn get_active_config_descriptor(&self) -> Result<ConfigDescriptor> {
        let mut descriptor: *mut bindings::libusb_config_descriptor = std::ptr::null_mut();
        // Safe because 'self.device' is valid and '&mut descriptor' is valid.
        handle_libusb_error!(unsafe {
            bindings::libusb_get_active_config_descriptor(
                self.device,
                &mut descriptor
                )
        });
        // Safe becuase descriptor points to valid memory.
        Ok(unsafe {
            ConfigDescriptor::new(descriptor)
        })
    }

    /// Get bus number of this device.
    pub fn get_bus_number(&self) -> u8 {
        // Safe because 'self.device' is valid.
        unsafe { bindings::libusb_get_bus_number(self.device) }
    }

    /// Get address of this device.
    pub fn get_address(&self) -> u8 {
        // Safe because 'self.device' is valid.
        unsafe { bindings::libusb_get_device_address(self.device) }
    }

    /// Get speed of this device.
    pub fn get_speed(&self) -> Speed {
        // Safe because 'self.device' is valid.
        let speed = unsafe { bindings::libusb_get_device_speed(self.device) };
        Speed::from(speed as u32)
    }

    /// Get device handle of this device.
    pub fn open(&self) -> Result<DeviceHandle> {
        let mut handle: *mut bindings::libusb_device_handle = std::ptr::null_mut();
        // Safe because 'self.device' is valid and handle is on stack.
        handle_libusb_error!(unsafe {
            bindings::libusb_open(self.device, &mut handle)
        });
        // Safe because handle points to valid memory.
        Ok(unsafe {
           DeviceHandle::new(self._context, handle)
        })
    }
}