// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::types::{EndpointDirection, EndpointType};
use bindings::libusb_endpoint_descriptor;
use std::ops::Deref;

/// ConfigDescriptor wraps libusb_interface_descriptor.
pub struct EndpointDescriptor<'a> {
    descriptor: &'a libusb_endpoint_descriptor,
}

const ENDPOINT_DESCRIPTOR_DIRECTION_MASK: u8 = 1 << 7;
const ENDPOINT_DESCRIPTOR_NUMBER_MASK: u8 = 0xf;
const ENDPOINT_DESCRIPTOR_ATTRIBUTES_TYPE_MASK: u8 = 0x3;

impl<'a> EndpointDescriptor<'a> {
    pub fn new(descriptor: &libusb_endpoint_descriptor) -> EndpointDescriptor {
        EndpointDescriptor { descriptor }
    }

    pub fn get_direction(&self) -> EndpointDirection {
        let direction = self.descriptor.bEndpointAddress & ENDPOINT_DESCRIPTOR_DIRECTION_MASK;
        if direction > 0 {
            EndpointDirection::DeviceToHost
        } else {
            EndpointDirection::HostToDevice
        }
    }

    pub fn get_endpoint_number(&self) -> u8 {
        self.descriptor.bEndpointAddress & ENDPOINT_DESCRIPTOR_NUMBER_MASK
    }

    pub fn get_endpoint_type(&self) -> Option<EndpointType> {
        let ep_type = self.descriptor.bmAttributes & ENDPOINT_DESCRIPTOR_ATTRIBUTES_TYPE_MASK;
        match ep_type {
            0 => Some(EndpointType::Control),
            1 => Some(EndpointType::Isochronous),
            2 => Some(EndpointType::Bulk),
            3 => Some(EndpointType::Interrupt),
            _ => None,
        }
    }
}

impl<'a> Deref for EndpointDescriptor<'a> {
    type Target = libusb_endpoint_descriptor;

    fn deref(&self) -> &libusb_endpoint_descriptor {
        self.descriptor
    }
}
