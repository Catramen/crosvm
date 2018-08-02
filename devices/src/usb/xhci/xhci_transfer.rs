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
    // The transfer completed with error.
    Error,
    // The transfer completed successfuly.
    Success,
}

pub struct XhciTransfer {
    xhci: Weak<Xhci>,
    transfer_completion_event: EventFd,
    ty: TransferType,
    endpoint_id: u8,
    transfer_trbs: TransferDescriptor,
}

impl XhciTransfer {
    pub fn new(xhci: &Weak<Xhci>,
               endpoint_id: u8,
               transfer_trbs: TransferDescriptor,
               completion_event: EventFd) -> Self {
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
            xhci: xhci.clone(),
            transfer_completion_event: completion_event,
            ty: TransferType,
            endpoint_id,
            transfer_trbs,
        }
    }

    pub fn submit_to_backend<T: XhciBackend>(self, backend: &T) {
        if self.is_valid() {
        } else {
            backend.submit_transfer();
        }
    }

    // TODO(jkwang) rewrite this part.
    pub fn on_transfer_complete(&self, status: TransferStatus, bytes_transferred: u32) {
        self.transfer_completion_event.write(1);
        let mut edtla: u32 = 0;
        // As noted in xHCI spec 4.11.3.1
        // Transfer Event Trb only occur under the following conditions:
        //   1. If the Interrupt On Completion flag is set.
        //   2. When a short tansfer occurs during the execution of a Transfer TRB and the
        //      Interrupter-on-Short Packet flag is set.
        //   3. If an error occurs during the execution of a Transfer Trb.
        for atrb in self.transfer_trbs {
            edtla += atrb.trb.transfer_length()
                if atrb.trb.interrupt_on_completion() {
                    // For details about event data trb and EDTLA, see spec 4.11.5.2.
                    if atrb.trb.trb_type() == TrbType::EventData {
                        let tlength: u32 = min(edtla, bytes_transferred);
                        self.send_transfer_event_trb(
                            TrbCompletionCode::Success,
                            atrb.trb.interrupter_target(),
                            atrb.trb.cast<EventDataTrb>().get_event_data(),
                            tlength,
                            true
                            );
                    } else {
                        // Short Transfer details, see xHCI spec 4.10.1.1.
                        let residual_transfer_length: u32 = edtla - bytes_transferred;
                        if edtla > bytes_transferred {
                            self.send_transfer_event_trb(
                                TrbCompletionCode::ShortPacket,
                                atrb.trb.interrupter_target(),
                                atrb.gpa.0,
                                residual_transfer_length,
                                true
                                );

                        } else {
                            self.send_transfer_event_trb(
                                TrbCompletionCode::Success,
                                atrb.trb.interrupter_target(),
                                atrb.gpa.0,
                                residual_transfer_length,
                                true
                                );
                        }
                    }
                }

        }
    }

    fn send_transfer_event_trb(&self,
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

    fn is_valid(atrb: &AddressedTrb, max_interrupters: u8) -> bool {
        atrb.trb.can_in_transfer_ring() &&
            (atrb.trb.interrupter_target() <= max_interrupters)
    }

    // Check each trb in the transfer descriptor for invalid or out of bounds
    // parameters. Returns true iff the transfer descriptor is valid.
    fn validate_trb(&self, max_interrupters: u32) -> Result<(), Vec<GuestAddress>> {
        let invalid_vec = Vec::new();
        for atrb in self.transfer_trbs {
            if !is_valid(atrb, max_interrupters) {
                invalid_vec.push(trb.gpa());
            }
        }
        if invalid_vec.is_empty() {
            Ok(())
        } else {
            Err(invalid_vec)
        }
    }


}

