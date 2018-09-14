// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex};

use usb::xhci::xhci_transfer::{XhciTransfer, EndpointDirection};
use usb_util::types::EndpointType;
use usb_util::device_handle::DeviceHandle;

pub struct UsbEndpoint {
    device_handle: Arc<Mutex<DeviceHandle>>,
    endpoint_number: u8,
    direction: EndpointDirection,
    ty: EndpointType,
}

impl UsbEndpoint {
    pub fn new(device_handle: Arc<Mutex<DeviceHandle>>,
               endpoint_number: u8,
               direction: EndpointDirection,
               ty: EndpointType
               ) -> UsbEndpoint {
        UsbEndpoint {
            device_handle,
            endpoint_number,
            direction,
            ty,
        }
    }

    pub fn match_ep(&self, endpoint_number: u8, dir: &EndpointDirection) -> bool {
        (self.endpoint_number == endpoint_number) && (self.direction == *dir)
    }

    pub fn handle_transfer(&self, transfer: XhciTransfer) {
    }
}
