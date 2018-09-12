// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::xhci_transfer::XhciTransfer;

/// Address of this usb device.
pub type UsbDeviceAddress = u32;

/// Xhci backend device is a virtual device connected to xHCI controller. It handles xhci transfers.
pub trait XhciBackendDevice: Send + Sync {
    /// Submit a xhci transfer to backend.
    fn submit_transfer(&self, transfer: XhciTransfer);
    /// Set address of this backend.
    fn set_address(&self, address: UsbDeviceAddress);
}
