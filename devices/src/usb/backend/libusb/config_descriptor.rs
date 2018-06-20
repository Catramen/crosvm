// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use usb::libusb::bindings::*;

pub struct ConfigDescriptor {
    descriptor: *mut libusb_config_descriptor,
}

impl Drop for ConfigDescriptor {
    fn drop(&mut self) {
        unsafe {
            libusb_free_config_descriptor(self.descriptor);
        }
    }
}

impl ConfigDescriptor {
    pub fn new(descriptor: *mut libusb_config_descriptor) -> ConfigDescriptor {
        ConfigDescriptor {
            descriptor: descriptor,
        }
    }
}

