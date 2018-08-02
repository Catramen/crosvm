// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use usb::event_loop::{EventLoop, EventHandler};
use sys_util::{GuestAddress, GuestMemory};
use super::xhci_abi::{
    AddressedTrb,
    TrbCompletionCode,
};

use super::ring_buffer::RingBuffer;
use super::ring_buffer::RingBufferController;
use super::xhci_transfer::XhciTransfer;

pub type TransferRingController = RingBufferController<TransferRingTrbHandler>;

struct TransferRingTrbHandler {
    xhci: Weak<Xhci>,
    slot_id: u8,
    endpoint_id: u8,
    backend: Arc<UsbBackend>,
}

impl TransferDescriptorHandler for TransferRingTrbHandler {
    fn handle_transfer_descriptor(&self,
                                  descriptor: TransferDescriptor,
                                  completion_event: EventFd) {
        let xhci_transfer = XhciTransfer::new(
            self.xhci,
            self.endpoint_id,
            descriptor,
            completion_event.try_clone().unwrap(),
            );
        xhci_transfer.submit_to_backend(&self.backend);
    }
}

impl TransferRingController {
    pub fn new() -> TransferRingController {
    }
}

