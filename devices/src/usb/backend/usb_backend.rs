// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

type UsbDeviceAddress = u32;

pub struct UsbBackend {
    address: UsbDeviceAddress,
}

pub trait UsbBackendInterface {
    // Submits the transfer request to t he exectuion.
    fn submit_transfer(&self, transfer: UsbTransfer);

    // Requests the cancellation of the execution of the previously submitted
    // transfer.
    fn cancel_transfer(&self, transfer: UsbTransfer);

    // Assigns address to the backend.
    fn set_address(&self, address: UsbDeviceAddress);

    // Returns the address of the backend.
    fn address(&self) -> UsbDeviceAddress;

    // Creates new endpoint.
    // TODO(jkwang) figure out endpoint ownership.
    fn create_endpoint(&self, pid: u8, endpoint: u8, ty: UsbEndpointType) -> UsbEndpoint;

    // Delete all end points except endpoint 0.
    fn delete_all_endpoints(&self);
}
