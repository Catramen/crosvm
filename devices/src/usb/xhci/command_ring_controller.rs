// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Weak};
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use usb::event_loop::{EventLoop, EventHandler};
use sys_util::{GuestAddress, GuestMemory};
use super::xhci_abi::{
    AddressedTrb,
    TrbCompletionCode,
};

use super::ring_buffer::RingBuffer;
use super::ring_buffer::RingBufferController;

struct CommandRingTrbHandler {
    xhci: Weak<Xhci>,
}

impl CommandRingTrbHandler {
    fn xhci(&self) -> Arc<Xhci> {
        self.xhci.upgrade().unwrap()
    }

    fn enable_slot(&self, atrb: &AddressedTrb) {
        let xhci = self.xhci();
        for i in 0..xhci.max_slots() {
            if xhci.device_slot(i).enable() {
                // slot id starts from 1.
                self.send_command_completion_trb(TrbCompletionCode::Success, i + 1, trb.gpa);
                return;
            }
        }
        self.send_command_completion_trb(TrbCompletionCode::NoSlotsAvailableError, 0, trb.gpa);
    }

    fn disable_slot(&self, atrb: &AddressedTrb) {
        let xhci = self.xhci();
        let trb = atrb.trb.cast::<DisableSlotCommandTrb>();
        let slot_id = trb.get_slot_id();
         match xhci.device_slot(slot_id) {
             Some(slot) -> {
                 self.send_command_completion_trb(dev_slot.disable(), slot_id, atrb.gpa);
             },
             None -> {
                 self.send_command_completion_trb(TrbCompletionCode::TrbError, slot_id, atrb.gpa);
             },
         }

    }

    fn address_device(&self, atrb: &AddressedTrb) {
        let trb = atrb.trb.cast::<AddressDeviceCommandTrb>();
        let slot = self.xhci().device_slot(trb.get_slot_id());
        match slot {
            Some(slot) -> {
                self.send_command_completion_trb(slot.set_address(trb),
                                                 trb.get_slot_id(),
                                                 atrb.gpa);
            },
            None -> {
                self.send_command_completion_trb(TrbCompletionCode::TrbError,
                                                 trb.get_slot_id(), atrb.gpa);
            }
        }
    }

    fn configure_endpoint(&self, atrb: &AddressedTrb) {
        let trb = atrb.trb.cast::<ConfigureEndpointCommandTrb>();
        let slot = self.xhci().device_slot(trb.get_slot_id());
        match slot {
            Some(slot) -> {
                self.send_command_completion_trb(slot.configure_endpoint(trb),
                                                 trb.get_slot_id(),
                                                 atrb.gpa);
            },
            None -> {
                self.send_command_completion_trb(TrbCompletionCode::TrbError,
                                                 trb.get_slot_id(),
                                                 atrb.gpa);
            }
        }
    }

    fn evaluate_context(&self, trb: &AddressedTrb) {
        let trb = atrb.trb.cast::<ConfigureEndpointCommandTrb>();
        let slot = self.xhci().device_slot(trb.get_slot_id());
        match slot {
            Some(slot) -> {
                self.send_command_completion_trb(slot.evaluate_context(trb),
                trb.get_slot_id(),
                atrb.gpa);
            },
            None -> {
                self.send_command_completion_trb(TrbCompletionCode::TrbError,
                                                 trb.get_slot_id(),
                                                 atrb.gpa);
            }
        }
    }

    fn reset_device(&self, trb: AddressedTrb) {
        let trb = atrb.trb.cast::<ResetDeviceCommandTrb>();
        let slot = self.xhci().device_slot(trb.get_slot_id());
        match slot {
            Some(slot) -> {
                self.send_command_completion_trb(slot.reset_device(trb),
                trb.get_slot_id(),
                atrb.gpa);
            },
            None -> {
                self.send_command_completion_trb(TrbCompletionCode::TrbError,
                                                 trb.get_slot_id(),
                                                 atrb.gpa);
            }
        }
    }

    fn send_command_completion_trb(&self,
                                   completion_code: TrbCompletionCode,
                                   slot_id: u8,
                                   trb_addr: GuestAddress) {
        let mut trb = CommandCompletionEventTrb::new();
        trb.set_trb_pointer(trb_addr);
        trb.set_command_completion_parameter(0);
        trb.set_completion_code(completion_code as u8);
        trb.set_trb_type(TrbType:: completionEvent);
        trb.set_vf_id(0);
        trb.set_slot_id(slot_id);
        self.xhci.upgrade().unwrap().interrupter(0).add_event(trb.cast<Trb>());
    }
}

impl TransferDescriptorHandler for CommandRingTrbHandler {
    fn handle_transfer_descriptor(&self, descriptor: TransferDescriptor, complete_event: EventFd) {
        // Command descriptor always consist of a single TRB.
        assert_eq!(descriptor.len());
        let atrb = &descriptor[0];
        match atrb.trb.trb_type() {
            Some(TrbType::EnableSlotCommand) => self.enable_slot(atrb),
            Some(TrbType::DisableSlotCommand) => self.disable_slot(atrb),
            Some(TrbType::AddressDeviceCommand) => self.address_device(atrb),
            Some(TrbType::ConfigureEndpointCommand) => self.configure_endpoint(atrb),
            Some(TrbType::EvaluateContextCommand) => self.evaluate_context(atrb),
            Some(TrbType::ResetDeviceCommand) => self.reset_device(atrb),
            Some(TrbType::NoopCommand) =>
                self.send_command_completion_trb(TrbCompletionCode::Success, 0, atrb.gpa),
            _ => warn!("Unexpected command ring trb type: {}", atrb.get_trb_type()),
        }
        complete_event.write(1);
    }
}
