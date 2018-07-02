// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use bindings::libusb_device_descriptor;

/// DeviceDescriptor wraps libusb_device_descriptor.
pub struct DeviceDescriptor {
    descriptor: libusb_device_descriptor,
}

impl DeviceDescriptor {
    // Create a new DeviceDescriptor.
    // TODO(jkwang) add more functions related to device descriptor.
    pub fn new(descriptor: libusb_device_descriptor) -> DeviceDescriptor {
        DeviceDescriptor {
            descriptor: descriptor,
        }
    }
}
