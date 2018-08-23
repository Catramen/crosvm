// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::interrupter::Interrupter;
use super::xhci_abi::*;
use super::xhci_backend_device::XhciBackendDevice;
use super::xhci_regs::MAX_INTERRUPTER;
use std::cmp::min;
use std::sync::{Arc, Mutex};
use sys_util::EventFd;
use usb_util::types::TransferType;

/// Status of this transfer.
pub enum TransferStatus {
    // The transfer completed with error.
    Error,
    // The transfer completed successfuly.
    Success,
}

/// Xhci transfer denote a transfer initiated by guest os driver. It will be submited to a
/// XhciBackendDevice.
pub struct XhciTransfer {
    interrupter: Arc<Mutex<Interrupter>>,
    ty: TransferType,
    slot_id: u8,
    endpoint_id: u8,
    transfer_trbs: TransferDescriptor,
    transfer_completion_event: EventFd,
}

impl XhciTransfer {
    /// Build a new XhciTransfer.
    pub fn new(
        interrupter: Arc<Mutex<Interrupter>>,
        slot_id: u8,
        endpoint_id: u8,
        transfer_trbs: TransferDescriptor,
        completion_event: EventFd,
    ) -> Self {
        assert!(transfer_trbs.len() > 0);
        let first_trb = transfer_trbs[0].trb;
        // For more information about transfer types, refer to xHCI spec 3.2.9 and 3.2.10.
        let transfer_type = match first_trb.trb_type() {
            Some(TrbType::Normal) => {
                if endpoint_id % 2 == 0 {
                    TransferType::Out
                } else {
                    TransferType::In
                }
            }
            Some(TrbType::SetupStage) => TransferType::Setup,
            Some(TrbType::DataStage) => {
                if first_trb.cast::<DataStageTrb>().get_direction() == 0 {
                    TransferType::Out
                } else {
                    TransferType::In
                }
            }
            Some(TrbType::StatusStage) => {
                if first_trb.cast::<StatusStageTrb>().get_direction() == 0 {
                    TransferType::Out
                } else {
                    TransferType::In
                }
            }
            _ => panic!("Invalid trb type"),
        };
        XhciTransfer {
            interrupter,
            transfer_completion_event: completion_event,
            ty: transfer_type,
            slot_id,
            endpoint_id,
            transfer_trbs,
        }
    }

    /// This functions should be invoked when transfer is completed (or failed).
    pub fn on_transfer_complete(&self, status: TransferStatus, bytes_transferred: u32) {
        self.transfer_completion_event.write(1);
        let mut edtla: u32 = 0;
        // As noted in xHCI spec 4.11.3.1
        // Transfer Event Trb only occur under the following conditions:
        //   1. If the Interrupt On Completion flag is set.
        //   2. When a short tansfer occurs during the execution of a Transfer TRB and the
        //      Interrupter-on-Short Packet flag is set.
        //   3. If an error occurs during the execution of a Transfer Trb.
        for atrb in &self.transfer_trbs {
            edtla += atrb.trb.transfer_length();
            if atrb.trb.interrupt_on_completion() {
                // For details about event data trb and EDTLA, see spec 4.11.5.2.
                if atrb.trb.trb_type().unwrap() == TrbType::EventData {
                    let tlength: u32 = min(edtla, bytes_transferred);
                    self.send_transfer_event_trb(
                        TrbCompletionCode::Success,
                        atrb.trb.cast::<EventDataTrb>().get_event_data(),
                        tlength,
                        true,
                    );
                } else {
                    // For Short Transfer details, see xHCI spec 4.10.1.1.
                    let residual_transfer_length: u32 = edtla - bytes_transferred;
                    if edtla > bytes_transferred {
                        self.send_transfer_event_trb(
                            TrbCompletionCode::ShortPacket,
                            atrb.gpa,
                            residual_transfer_length,
                            true,
                        );
                    } else {
                        self.send_transfer_event_trb(
                            TrbCompletionCode::Success,
                            atrb.gpa,
                            residual_transfer_length,
                            true,
                        );
                    }
                }
            }
        }
    }

    pub fn send_to_backend_if_valid(self, backend: &XhciBackendDevice) {
        if self.validate_transfer() {
            // Backend should invoke on transfer complete when transfer is completed.
            backend.submit_transfer(self);
        } else {
            self.transfer_completion_event.write(1);
        }
    }

    // Check each trb in the transfer descriptor for invalid or out of bounds
    // parameters. Returns true iff the transfer descriptor is valid.
    fn validate_transfer(&self) -> bool {
        let mut valid = true;
        for atrb in &self.transfer_trbs {
            if !Self::trb_is_valid(&atrb) {
                self.send_transfer_event_trb(TrbCompletionCode::TrbError, atrb.gpa, 0, false);
                valid = false;
            }
        }
        valid
    }

    fn trb_is_valid(atrb: &AddressedTrb) -> bool {
        atrb.trb.can_be_in_transfer_ring() && (atrb.trb.interrupter_target() < MAX_INTERRUPTER)
    }

    fn send_transfer_event_trb(
        &self,
        completion_code: TrbCompletionCode,
        trb_pointer: u64,
        transfer_length: u32,
        event_data: bool,
    ) {
        let mut trb = Trb::new();
        {
            let event_trb = trb.cast_mut::<TransferEventTrb>();
            event_trb.set_trb_pointer(trb_pointer);
            event_trb.set_trb_transfer_length(transfer_length);
            event_trb.set_completion_code(completion_code as u8);
            event_trb.set_event_data(event_data.into());
            event_trb.set_trb_type(TrbType::TransferEvent as u8);
            event_trb.set_endpoint_id(self.endpoint_id);
            event_trb.set_slot_id(self.slot_id);
        }
        self.interrupter.lock().unwrap().add_event(trb);
    }
}
