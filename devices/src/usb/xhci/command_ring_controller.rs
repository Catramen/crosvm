// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::device_slot::{DeviceSlot, DeviceSlots};
use super::interrupter::Interrupter;
use super::ring_buffer::RingBuffer;
use super::ring_buffer_controller::{RingBufferController, TransferDescriptorHandler};
use super::xhci::Xhci;
use super::xhci_abi::*;
use super::xhci_regs::MAX_SLOTS;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use sys_util::{EventFd, GuestAddress, GuestMemory};
use usb::event_loop::{EventHandler, EventLoop};

pub type CommandRingController = RingBufferController<CommandRingTrbHandler>;

impl CommandRingController {
    pub fn new(
        mem: GuestMemory,
        event_loop: EventLoop,
        slots: DeviceSlots,
        interrupter: Arc<Mutex<Interrupter>>,
    ) -> Arc<CommandRingController> {
        RingBufferController::create_controller(
            mem,
            event_loop,
            CommandRingTrbHandler::new(slots, interrupter),
        )
    }
}

pub struct CommandRingTrbHandler {
    slots: DeviceSlots,
    interrupter: Arc<Mutex<Interrupter>>,
}

impl CommandRingTrbHandler {
    fn new(slots: DeviceSlots, interrupter: Arc<Mutex<Interrupter>>) -> Self {
        CommandRingTrbHandler { slots, interrupter }
    }

    // Slot idex + 1= Slot id
    fn slot(&self, slot_index: u8) -> MutexGuard<DeviceSlot> {
        self.slots.slot(slot_index).unwrap()
    }

    fn enable_slot(&self, atrb: &AddressedTrb, event_fd: EventFd) {
        debug!("running enable slot command ");
        for i in 0..MAX_SLOTS {
            if self.slot(i).enable() {
                // Slot id starts from 1.
                self.interrupter
                    .lock()
                    .unwrap()
                    .send_command_completion_trb(
                        TrbCompletionCode::Success,
                        i + 1,
                        GuestAddress(atrb.gpa),
                    );
                event_fd.write(1).unwrap();
                return;
            }
        }
        self.interrupter
            .lock()
            .unwrap()
            .send_command_completion_trb(
                TrbCompletionCode::NoSlotsAvailableError,
                0,
                GuestAddress(atrb.gpa),
            );
        event_fd.write(1).unwrap();
    }

    fn disable_slot(&self, atrb: &AddressedTrb, event_fd: EventFd) {
        let trb = atrb.trb.cast::<DisableSlotCommandTrb>();
        let slot_id = trb.get_slot_id();
        if slot_id < MAX_SLOTS {
            self.slots.disable_slot(slot_id, atrb, event_fd);
        } else {
            self.interrupter
                .lock()
                .unwrap()
                .send_command_completion_trb(
                    TrbCompletionCode::TrbError,
                    slot_id,
                    GuestAddress(atrb.gpa),
                );
            event_fd.write(1).unwrap();
        }
    }

    fn address_device(&self, atrb: &AddressedTrb, event_fd: EventFd) {
        let trb = atrb.trb.cast::<AddressDeviceCommandTrb>();
        let slot_id = trb.get_slot_id();
        if slot_id < MAX_SLOTS {
            self.interrupter
                .lock()
                .unwrap()
                .send_command_completion_trb(
                    self.slot(slot_id).set_address(trb),
                    slot_id,
                    GuestAddress(atrb.gpa),
                );
        } else {
            self.interrupter
                .lock()
                .unwrap()
                .send_command_completion_trb(
                    TrbCompletionCode::TrbError,
                    slot_id,
                    GuestAddress(atrb.gpa),
                );
        }
        event_fd.write(1).unwrap();
    }

    fn configure_endpoint(&self, atrb: &AddressedTrb, event_fd: EventFd) {
        let trb = atrb.trb.cast::<ConfigureEndpointCommandTrb>();
        let slot_id = trb.get_slot_id();
        if slot_id < MAX_SLOTS {
            self.interrupter
                .lock()
                .unwrap()
                .send_command_completion_trb(
                    self.slot(slot_id).configure_endpoint(trb),
                    slot_id,
                    GuestAddress(atrb.gpa),
                );
        } else {
            self.interrupter
                .lock()
                .unwrap()
                .send_command_completion_trb(
                    TrbCompletionCode::TrbError,
                    slot_id,
                    GuestAddress(atrb.gpa),
                );
        }
        event_fd.write(1).unwrap();
    }

    fn evaluate_context(&self, atrb: &AddressedTrb, event_fd: EventFd) {
        let trb = atrb.trb.cast::<EvaluateContextCommandTrb>();
        let slot_id = trb.get_slot_id();
        if slot_id < MAX_SLOTS {
            self.interrupter
                .lock()
                .unwrap()
                .send_command_completion_trb(
                    self.slot(slot_id).evaluate_context(trb),
                    slot_id,
                    GuestAddress(atrb.gpa),
                );
        } else {
            self.interrupter
                .lock()
                .unwrap()
                .send_command_completion_trb(
                    TrbCompletionCode::TrbError,
                    slot_id,
                    GuestAddress(atrb.gpa),
                );
        }
        event_fd.write(1).unwrap();
    }

    fn reset_device(&self, atrb: &AddressedTrb, event_fd: EventFd) {
        let trb = atrb.trb.cast::<ResetDeviceCommandTrb>();
        let slot_id = trb.get_slot_id();
        if slot_id < MAX_SLOTS {
            self.slots.reset_slot(slot_id, atrb, event_fd);
        } else {
            self.interrupter
                .lock()
                .unwrap()
                .send_command_completion_trb(
                    TrbCompletionCode::TrbError,
                    slot_id,
                    GuestAddress(atrb.gpa),
                );
        }
    }
}

impl TransferDescriptorHandler for CommandRingTrbHandler {
    fn handle_transfer_descriptor(&self, descriptor: TransferDescriptor, complete_event: EventFd) {
        // Command descriptor always consist of a single TRB.
        assert_eq!(descriptor.len(), 1);
        let atrb = &descriptor[0];
        match atrb.trb.trb_type() {
            Some(TrbType::EnableSlotCommand) => self.enable_slot(atrb, complete_event),
            Some(TrbType::DisableSlotCommand) => self.disable_slot(atrb, complete_event),
            Some(TrbType::AddressDeviceCommand) => self.address_device(atrb, complete_event),
            Some(TrbType::ConfigureEndpointCommand) => {
                self.configure_endpoint(atrb, complete_event)
            }
            Some(TrbType::EvaluateContextCommand) => self.evaluate_context(atrb, complete_event),
            Some(TrbType::ResetDeviceCommand) => self.reset_device(atrb, complete_event),
            Some(TrbType::NoopCommand) => {
                self.interrupter
                    .lock()
                    .unwrap()
                    .send_command_completion_trb(
                        TrbCompletionCode::Success,
                        0,
                        GuestAddress(atrb.gpa),
                    );
                complete_event.write(1);
            }
            _ => warn!(
                "Unexpected command ring trb type: {}",
                atrb.trb.get_trb_type()
            ),
        }
    }
}
