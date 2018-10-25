// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::endpoint_descriptor::EndpointDescriptor;
use bindings::libusb_interface_descriptor;
use std::ops::Deref;

/// ConfigDescriptor wraps libusb_interface_descriptor.
pub struct InterfaceDescriptor<'a> {
    descriptor: &'a libusb_interface_descriptor,
}

impl<'a> InterfaceDescriptor<'a> {
    pub fn new(descriptor: &'a libusb_interface_descriptor) -> InterfaceDescriptor<'a> {
        InterfaceDescriptor { descriptor }
    }

    pub fn endpoint_descriptor(&self, ep_idx: u8) -> Option<EndpointDescriptor> {
        if ep_idx >= self.descriptor.bNumEndpoints {
            return None;
        }

        unsafe {
            Some(EndpointDescriptor::new(
                &*(self.descriptor.endpoint.offset(ep_idx as isize)),
            ))
        }
    }
}

impl<'a> Deref for InterfaceDescriptor<'a> {
    type Target = libusb_interface_descriptor;

    fn deref(&self) -> &libusb_interface_descriptor {
        self.descriptor
    }
}
