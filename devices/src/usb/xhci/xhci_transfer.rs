// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::interrupter::Interrupter;
use super::scatter_gather_buffer::ScatterGatherBuffer;
use super::usb_hub::UsbPort;
use super::xhci_abi::*;
use super::xhci_backend_device::XhciBackendDevice;
use super::xhci_regs::MAX_INTERRUPTER;
use std::cmp::min;
use std::mem::swap;
use std::sync::{Arc, Weak, Mutex};
use sys_util::{EventFd, GuestMemory};
use usb_util::types::UsbRequestSetup;
use usb_util::usb_transfer::TransferStatus;

/// Type of usb endpoints.
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum TransferDirection {
    In,
    Out,
    Control,
}

/// Current state of xhci transfer.
pub enum XhciTransferState {
    Created,
    Submitted(Box<Fn() + Send>),
    Cancelling,
    Cancelled,
    Completed,
}

impl XhciTransferState {
    /// Try to cancel this transfer, if it's possible.
    pub fn try_cancel(&mut self) {
        let mut tmp = XhciTransferState::Created;
        swap(&mut tmp, self);
        match tmp {
            XhciTransferState::Submitted(cb) => {
                *self = XhciTransferState::Cancelling;
                cb();
            },
            XhciTransferState::Cancelling => {
                error!("Another cancellation is already issued.");
            }
            _ => {
                *self = XhciTransferState::Cancelled;
            }
        }
    }
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
            }
            TrbType::SetupStage => {
                let trb = td[0].trb.cast::<SetupStageTrb>();
                XhciTransferType::SetupStage(UsbRequestSetup::new(
                    trb.get_request_type(),
                    trb.get_request(),
                    trb.get_value(),
                    trb.get_index(),
                    trb.get_length(),
                ))
            }
            TrbType::DataStage => {
                let buffer = ScatterGatherBuffer::new(mem, td);
                XhciTransferType::DataStage(buffer)
            }
            TrbType::StatusStage => XhciTransferType::StatusStage,
            TrbType::Isoch => {
                let buffer = ScatterGatherBuffer::new(mem, td);
                XhciTransferType::Isoch(buffer)
            }
            TrbType::Noop => XhciTransferType::Noop,
            _ => {
                panic!("Wrong trb type in transfer ring");
            }
        }
    }
}

/// Xhci Transfer manager holds reference to all on going transfers. Can cancell them all if
/// needed.
#[derive(Clone)]
pub struct XhciTransferManager {
    transfers: Arc<Mutex<Vec<Weak<Mutex<XhciTransferState>>>>>,
}

impl XhciTransferManager {
    /// Create a new manager.
    pub fn new() -> XhciTransferManager {
        XhciTransferManager {
            transfers: Arc::new(Mutex::new(Vec::new()))
        }
    }

    /// Build a new XhciTransfer. Endpoint id is the id in xHCI device slot.
    pub fn create_transfer(&self,
                           mem: GuestMemory,
                           port: Arc<UsbPort>,
                           interrupter: Arc<Mutex<Interrupter>>,
                           slot_id: u8,
                           endpoint_id: u8,
                           transfer_trbs: TransferDescriptor,
                           completion_event: EventFd,
                           ) -> XhciTransfer {
        assert!(transfer_trbs.len() > 0);
        let transfer_dir = {
            if endpoint_id == 0 {
                TransferDirection::Control
            } else if (endpoint_id % 2) == 0 {
                TransferDirection::Out
            } else {
                TransferDirection::In
            }
        };
        let t = XhciTransfer {
            manager: self.clone(),
            state: Arc::new(Mutex::new(XhciTransferState::Created)),
            mem,
            port,
            interrupter,
            transfer_completion_event: completion_event,
            slot_id,
            endpoint_id,
            transfer_dir,
            transfer_trbs,
        };
        self.transfers.lock().unwrap().push(Arc::downgrade(&t.state));
        t
    }

    pub fn remove_transfer(&self, t: &Arc<Mutex<XhciTransferState>>) {
        let mut transfers = self.transfers.lock().unwrap();
        match transfers.iter().position(|ref wt|{
            Arc::ptr_eq(&wt.upgrade().unwrap(), t)
        }) {
            None => error!("Try removing unknow transfer"),
            Some(i) => {
                transfers.swap_remove(i);
            }
        };
    }

    pub fn cancell_all(&self) {
        self.transfers.lock().unwrap().iter().for_each(
            |ref t| {
                let state = t.upgrade().unwrap();
                state.lock().unwrap().try_cancel();
            }
            );
    }
}

/// Xhci transfer denote a transfer initiated by guest os driver. It will be submited to a
/// XhciBackendDevice.
pub struct XhciTransfer {
    manager: XhciTransferManager,
    state: Arc<Mutex<XhciTransferState>>,
    mem: GuestMemory,
    port: Arc<UsbPort>,
    interrupter: Arc<Mutex<Interrupter>>,
    slot_id: u8,
    // id of endpoint in device slot.
    endpoint_id: u8,
    transfer_dir: TransferDirection,
    transfer_trbs: TransferDescriptor,
    transfer_completion_event: EventFd,
}

impl Drop for XhciTransfer {
    fn drop(&mut self) {
        self.manager.remove_transfer(&self.state);
    }
}

impl XhciTransfer {
    pub fn print(&self) {
        debug!(
            "xhci_transfer slot id: {}, endpoint id {}, transfer_dir {:?}, transfer_trbs {:?}",
            self.slot_id, self.endpoint_id, self.transfer_dir, self.transfer_trbs
        );
    }

    /// Get state of this transfer.
    pub fn state(&self) -> &Arc<Mutex<XhciTransferState>> {
        &self.state
    }


    pub fn get_transfer_type(&self) -> XhciTransferType {
        XhciTransferType::new(self.mem.clone(), self.transfer_trbs.clone())
    }

    pub fn get_endpoint_number(&self) -> u8 {
        self.endpoint_id / 2
    }

    pub fn get_transfer_dir(&self) -> TransferDirection {
        self.transfer_dir
    }

    pub fn get_first_trb_as<T: TrbCast>(&self) -> Option<&T> {
        self.transfer_trbs[0].trb.checked_cast::<T>()
    }

    /// This functions should be invoked when transfer is completed (or failed).
    pub fn on_transfer_complete(&self, status: TransferStatus, bytes_transferred: u32) {
        match status {
            TransferStatus::NoDevice => {
                debug!("device disconnected, detaching from port");
                self.port.detach();
                // If the device is gone, we don't need to send transfer completion event, cause we
                // are going to destroy everything related to this device anyway.
                return;
            },
            TransferStatus::Cancelled => {
                // TODO(jkwang) According to the spec, we should send a stopped event here. But kernel driver
                // does not do anything meaningful when it sees a stopped event.
                self.transfer_completion_event.write(1).unwrap();
                return;
            },
            TransferStatus::Completed => {
                self.transfer_completion_event.write(1).unwrap();
            },
            _ => {
                // Transfer failed, we are not handling this correctly yet. Guest kernel might see
                // short packets for in transfer and might think control transfer is successful. It
                // will eventually find out device is in a wrong state.
                self.transfer_completion_event.write(1).unwrap();
            }
        }

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
                    debug!("on transfer complete event data");
                    let tlength: u32 = min(edtla, bytes_transferred);
                    self.interrupter.lock().unwrap().send_transfer_event_trb(
                        TrbCompletionCode::Success,
                        atrb.trb.cast::<EventDataTrb>().get_event_data(),
                        tlength,
                        true,
                        self.slot_id,
                        self.endpoint_id,
                    );
                } else {
                    // For Short Transfer details, see xHCI spec 4.10.1.1.
                    let residual_transfer_length: u32 = edtla - bytes_transferred;
                    if edtla > bytes_transferred {
                        debug!("on transfer complete short packet");
                        self.interrupter.lock().unwrap().send_transfer_event_trb(
                            TrbCompletionCode::ShortPacket,
                            atrb.gpa,
                            residual_transfer_length,
                            true,
                            self.slot_id,
                            self.endpoint_id,
                        );
                    } else {
                        debug!("on transfer complete success");
                        self.interrupter.lock().unwrap().send_transfer_event_trb(
                            TrbCompletionCode::Success,
                            atrb.gpa,
                            residual_transfer_length,
                            true,
                            self.slot_id,
                            self.endpoint_id,
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
                self.transfer_completion_event.write(1).unwrap();
                return;
            }
            backend.as_mut().unwrap().submit_transfer(self);
        } else {
            error!("invalid td on transfer ring");
            self.transfer_completion_event.write(1).unwrap();
        }
    }

    // Check each trb in the transfer descriptor for invalid or out of bounds
    // parameters. Returns true iff the transfer descriptor is valid.
    fn validate_transfer(&self) -> bool {
        let mut valid = true;
        for atrb in &self.transfer_trbs {
            if !Self::trb_is_valid(&atrb) {
                self.interrupter.lock().unwrap().send_transfer_event_trb(
                    TrbCompletionCode::TrbError,
                    atrb.gpa,
                    0,
                    false,
                    self.slot_id,
                    self.endpoint_id,
                );
                valid = false;
            }
        }
        valid
    }

    fn trb_is_valid(atrb: &AddressedTrb) -> bool {
        atrb.trb.can_be_in_transfer_ring() && (atrb.trb.interrupter_target() < MAX_INTERRUPTER)
    } }
