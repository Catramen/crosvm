// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

pub type UsbDeviceAddress = u32;

/// Xhci backend is a virtual device connected to xHCI controller. It handles xhci transfers.
pub trait XhciBackend {
    fn submit_transfer(transfer: XhciTransfer);
    fn set_address(address: UsbDeviceAddress);
}
