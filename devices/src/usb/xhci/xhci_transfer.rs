// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Weak};
use super::xhci_abi::{Addressed, Trb, TrbType, TrbCompletionCode, TransferEventTrb};
use super::ring_buffer::TransferDescriptor;
use sys_util::{
    EventFd,
};
use usb_util::TransferType;

pub enum TransferStatus {
    // The transfer is built, but not sent yet.
    NotSent,
    // The transfer completed with error.
    Error,
    // The transfer completed successfuly.
    Success,
}

pub struct XhciTransfer {
    xhci: Weak<Xhci>,
    transfer_completion_event: EventFd,
    mem: GuestMemory,
    ty: TransferType,
    endpoint_id: u8,
    transfer_trbs: TransferDescriptor,
}

impl XhciTransfer {
    pub fn new(xhci: &Arc<Xhci>,
               mem: GuestMemory,
               endpoint_id: u8,
               transfer_trbs: TransferDescriptor) -> Self {
        assert!(transfer_trbs.len() > 0);
        let first_trb = transfer_trbs[0].trb;
        // For more information about transfer types, refer to xHCI spec 3.2.9 and 3.2.10.
        let transfer_type = match first_trb.trb_type() {
            TrbType::Normal => {
                if endpoint_id %2 == 0 {
                    TransferType::Out
                } else {
                    TransferType::In
                }
            },
            TrbType::SetupStage => TransferType::Setup,
            TrbType::DataStage => {
                if first_trb.cast::<DataStageTrb>().get_direction() == 0 {
                    TransferType::Out
                } else {
                    TransferType::In
                }
            },
            TrbType::StatusStage => {
                if first_trb.cast::<StatusStageTrb>().get_direction() == 0 {
                    TransferType::Out
                } else {
                    TransferType::In
                }
            },
            _ => panic!("Invalid trb type");
        }
        XhciTransfer {
            xhci: Arc::downgrade(xhci),
            mem,
            ty: TransferType,
            endpoint_id,
            transfer_trbs,
        }
    }

    pub fn is_valid(atrb: &AddressedTrb, max_interrupters: u8) -> bool {
        self.trb.can_in_transfer_ring() &&
            (self.trb.interrupter_target() <= max_interrupters)
    }

    // Check each trb in the transfer descriptor for invalid or out of bounds
    // parameters. Returns true iff the transfer descriptor is valid.
    pub fn validate_trb(&self, max_interrupters: u32) -> Result<(), Vec<GuestAddress>> {
        let invalid_vec = Vec::new();
        for trb in self.transfer_trbs {
            if !trb.is_valid() {
                invalid_vec.push(trb.gpa());
            }
        }
        if invalid_vec.is_empty() {
            Ok(())
        } else {
            Err(invalid_vec)
        }
    }

    pub fn on_transfer_complete(&self, bytes_transferred: u32) {
        self.transfer_completion_event.write(1);
        let mut edtla: u32 = 0;
        for trb in self.transfer_trbs {
        }
    }

    pub fn send_transfer_event_trb(&self,
                                   completion_code: TrbCompletionCode,
                                   interrupter_target: u16,
                                   trb_pointer: u64,
                                   transfer_length: u32,
                                   event_data: bool) {
        let mut trb = TransferEventTrb::new();
        event_trb.set_trb_pointer(trb_pointer);
        event_trb.set_trb_transfer_length(transfer_length);
        event_trb.set_completion_code(completion_code);
        event_trb.set_event_data(event_data);
        event_trb.set_trb_type(TrbType::TransferEvent as u8);
        event_trb.endpoint_id = endpoint_id_;
        event_trb.slot_id = slot_id_;
        self.xhci.upgrade().unwrap().interrupter(interrupter_target).add_event(trb.cast<Trb>());
    }

    pub fn update_status(&mut self, status: TransferStatus) {
    }
}

