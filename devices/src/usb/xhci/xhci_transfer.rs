// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::interrupter::Interrupter;
use super::xhci_abi::*;
use super::xhci_backend_device::XhciBackendDevice;
use super::xhci_regs::MAX_INTERRUPTER;
use super::usb_hub::UsbPort;
use std::cmp::min;
use std::sync::{Arc, Mutex};
use sys_util::{EventFd, GuestMemory};
use usb_util::types::UsbRequestSetup;
use usb_util::usb_transfer::TransferStatus;
use super::scatter_gather_buffer::ScatterGatherBuffer;

/// Type of usb endpoints.
#[derive(PartialEq, Clone, Copy)]
pub enum EndpointDirection {
    In,
    Out,
    Control,
}

/// Type of a transfer received handled by transfer ring.
pub enum XhciTransferType {
    // Normal means bulk transfer or interrupt transfer, depending on endpoint type.
    // See spec 4.11.2.1.
    Normal(ScatterGatherBuffer),
    // See usb spec for setup stage, data stage and status stage,
    // see xHCI spec 4.11.2.2 for corresponding trbs.
    SetupStage(UsbRequestSetup),
    DataStage(ScatterGatherBuffer),
    StatusStage,
    // See xHCI spec 4.11.2.3.
    Isoch(ScatterGatherBuffer),
    // See xHCI spec 6.4.1.4.
    Noop,
}

impl XhciTransferType {
    pub fn new(mem: GuestMemory, td: TransferDescriptor) -> XhciTransferType {
        match td[0].trb.trb_type().unwrap() {
            TrbType::Normal => {
                let buffer = ScatterGatherBuffer::new(mem, td);
                XhciTransferType::Normal(buffer)
            },
            TrbType::SetupStage => {
                let trb = td[0].trb.cast::<SetupStageTrb>();
                XhciTransferType::SetupStage(UsbRequestSetup::new(
                        trb.get_request_type(),
                        trb.get_request(),
                        trb.get_value(),
                        trb.get_index(),
                        trb.get_length(),
                        ))
            },
            TrbType::DataStage => {
                let buffer = ScatterGatherBuffer::new(mem, td);
                XhciTransferType::DataStage(buffer)
            },
            TrbType::StatusStage => {
                XhciTransferType::StatusStage
            },
            TrbType::Isoch => {
                let buffer = ScatterGatherBuffer::new(mem, td);
                XhciTransferType::Isoch(buffer)
            },
            TrbType::Noop => {
                XhciTransferType::Noop
            },
            _ => {
                panic!("Wrong trb type in transfer ring");
            }
        }
    }
}

/// Xhci transfer denote a transfer initiated by guest os driver. It will be submited to a
/// XhciBackendDevice.
pub struct XhciTransfer {
    mem: GuestMemory,
    port: Arc<UsbPort>,
    interrupter: Arc<Mutex<Interrupter>>,
    slot_id: u8,
    // id of endpoint in device slot.
    endpoint_id: u8,
    endpoint_dir: EndpointDirection,
    transfer_trbs: TransferDescriptor,
    transfer_completion_event: EventFd,
}

impl XhciTransfer {
    /// Build a new XhciTransfer. Endpoint id is the id in xHCI device slot.
    pub fn new(
        mem: GuestMemory,
        port: Arc<UsbPort>,
        interrupter: Arc<Mutex<Interrupter>>,
        slot_id: u8,
        endpoint_id: u8,
        transfer_trbs: TransferDescriptor,
        completion_event: EventFd,
    ) -> Self {
        assert!(transfer_trbs.len() > 0);
        let endpoint_dir = {
            if endpoint_id == 0 {
                EndpointDirection::Control
            } else if (endpoint_id % 2) == 0 {
                EndpointDirection::Out
            } else {
                EndpointDirection::In
            }
        };
        XhciTransfer {
            mem,
            port,
            interrupter,
            transfer_completion_event: completion_event,
            slot_id,
            endpoint_id,
            endpoint_dir,
            transfer_trbs,
        }
    }

    pub fn get_transfer_type(&self) -> XhciTransferType {
        XhciTransferType::new(self.mem.clone(), self.transfer_trbs.clone())
    }

    pub fn get_endpoint_number(&self) -> u8 {
        self.endpoint_id / 2
    }

    pub fn get_endpoint_dir(&self) -> EndpointDirection {
        self.endpoint_dir
    }

    pub fn get_first_trb_as<T: TrbCast>(&self) -> Option<&T> {
        self.transfer_trbs[0].trb.checked_cast::<T>()
    }

    /// This functions should be invoked when transfer is completed (or failed).
    pub fn on_transfer_complete(&self, status: TransferStatus, bytes_transferred: u32) {
        if let TransferStatus::NoDevice = status {
            debug!("device disconnected, detaching from port");
            self.port.detach();
            return;
        }
        self.transfer_completion_event.write(1);
        let mut edtla: u32 = 0;
        // TODO(jkwang) Send event based on Status.
        // As noted in xHCI spec 4.11.3.1
        // Transfer Event Trb only occurs under the following conditions:
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

    pub fn send_to_backend_if_valid(self) {
        if self.validate_transfer() {
            // Backend should invoke on transfer complete when transfer is completed.
            let port = self.port.clone();
            let mut backend = port.get_backend_device();
            if backend.is_none() {
                error!("backend is already disconnected");
                self.transfer_completion_event.write(1);
                return;
            }
            backend.as_mut().unwrap().submit_transfer(self);
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
