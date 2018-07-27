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

pub type TransferRingController = RingBufferController<TransferRingTrbHandler>;

struct TransferRingTrbHandler {
    xhci: Weak<Xhci>,
    slot_id: u8,
    endpoint_id: u8,
    backend: Arc<UsbBackend>,
}

impl TransferDescriptorHandler for TransferRingTrbHandler {
    fn handle_transfer_descriptor(&self, descriptor: TransferDescriptor, complete_event: EventFd) {

    }
}

