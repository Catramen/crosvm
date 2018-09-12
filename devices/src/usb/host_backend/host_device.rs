// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use usb::xhci::xhci_backend_device::{XhciBackendDevice, UsbDeviceAddress};
use usb::xhci::xhci_transfer::XhciTransfer;
use usb_util::device_handle::DeviceHandle;

pub struct HostDevice {
    device_handle: Arc<Mutex<DeviceHandle>>,
    control_endpoint: ControlEndpoint,
    endpoints: Vec<DataEndpoint>,
}

impl XhciBackendDevice for HostDevice {
    fn submit_transfer(&self, transfer: XhciTransfer) {
    }

    fn set_address(&self, address: UsbDeviceAddress) {
    }
}
