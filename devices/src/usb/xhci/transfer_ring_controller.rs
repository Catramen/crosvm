// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::xhci_abi::{AddressedTrb, TrbCompletionCode};
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::sync::{Arc, Mutex};
use sys_util::{EventFd, GuestAddress, GuestMemory};
use usb::event_loop::{EventHandler, EventLoop};
use usb::xhci::ring_buffer_controller::{RingBufferController, TransferDescriptorHandler};

use super::interrupter::Interrupter;
use super::xhci::Xhci;
use super::xhci_abi::TransferDescriptor;
use super::xhci_backend_device::XhciBackendDevice;
use super::xhci_transfer::XhciTransfer;

pub type TransferRingController = RingBufferController<TransferRingTrbHandler>;

pub struct TransferRingTrbHandler {
    mem: GuestMemory,
    interrupter: Arc<Mutex<Interrupter>>,
    slot_id: u8,
    endpoint_id: u8,
    backend: Arc<Mutex<XhciBackendDevice>>,
}

impl TransferDescriptorHandler for TransferRingTrbHandler {
    fn handle_transfer_descriptor(
        &self,
        descriptor: TransferDescriptor,
        completion_event: EventFd,
    ) {
        let xhci_transfer = XhciTransfer::new(
            self.mem.clone(),
            self.interrupter.clone(),
            self.slot_id,
            self.endpoint_id,
            descriptor,
            completion_event,
        );
        xhci_transfer.send_to_backend_if_valid(&mut *self.backend.lock().unwrap());
    }
}

impl TransferRingController {
    pub fn new(
        mem: GuestMemory,
        event_loop: &EventLoop,
        interrupter: Arc<Mutex<Interrupter>>,
        slot_id: u8,
        endpoint_id: u8,
        backend: Arc<Mutex<XhciBackendDevice>>,
    ) -> Arc<TransferRingController> {
        RingBufferController::create_controller(
            mem.clone(),
            event_loop,
            TransferRingTrbHandler {
                mem,
                interrupter,
                slot_id,
                endpoint_id,
                backend,
            },
        )
    }
}
