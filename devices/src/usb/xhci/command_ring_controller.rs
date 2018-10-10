// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::device_slot::{DeviceSlot, DeviceSlots};
use super::interrupter::Interrupter;
use super::ring_buffer::RingBuffer;
use super::ring_buffer_controller::{RingBufferController, TransferDescriptorHandler};
use super::xhci::Xhci;
use super::xhci_abi::*;
use super::xhci_regs::{MAX_SLOTS, valid_slot_id};
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
            String::from("command ring"),
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

    fn slot(&self, slot_id: u8) -> MutexGuard<DeviceSlot> {
        self.slots.slot(slot_id).unwrap()
    }

    fn command_completion_callback(interrupter: &Arc<Mutex<Interrupter>>, completion_code: TrbCompletionCode,
                                 slot_id: u8, trb_addr: u64, event_fd: &EventFd) {
        interrupter
            .lock()
            .unwrap()
            .send_command_completion_trb(
                completion_code,
                slot_id,
                GuestAddress(trb_addr),
            );
        event_fd.write(1).unwrap();
    }

    fn enable_slot(&self, atrb: &AddressedTrb, event_fd: EventFd) {
        for i in 0..MAX_SLOTS {
            let slot_id = i + 1;
            if self.slot(slot_id).enable() {
                // Slot id starts from 1.
                debug!("running enable slot command slot_id {}", i);
                CommandRingTrbHandler::command_completion_callback(&self.interrupter,
                                                                   TrbCompletionCode::Success,
                                                                   slot_id, atrb.gpa, &event_fd);
                return;
            }
        }
        CommandRingTrbHandler::command_completion_callback(&self.interrupter,
                                                           TrbCompletionCode::NoSlotsAvailableError,
                                                           0, atrb.gpa, &event_fd);
    }

    fn disable_slot(&self, atrb: &AddressedTrb, event_fd: EventFd) {
        debug!("disabling slot");
        let trb = atrb.trb.cast::<DisableSlotCommandTrb>();
        let slot_id = trb.get_slot_id();
        if valid_slot_id(slot_id) {
            let gpa = atrb.gpa;
            let interrupter = self.interrupter.clone();
            self.slots.disable_slot(slot_id, move |completion_code | {
                CommandRingTrbHandler::command_completion_callback(&interrupter,
                                                                   completion_code,
                                                                   slot_id, gpa, &event_fd);
            });
        } else {
            CommandRingTrbHandler::command_completion_callback(&self.interrupter,
                                                               TrbCompletionCode::TrbError,
                                                               slot_id, atrb.gpa, &event_fd);
        }
    }

    fn address_device(&self, atrb: &AddressedTrb, event_fd: EventFd) {
        debug!("addressing device");
        let trb = atrb.trb.cast::<AddressDeviceCommandTrb>();
        let slot_id = trb.get_slot_id();
        let completion_code = {
            if valid_slot_id(slot_id) {
                self.slot(slot_id).set_address(trb)
            } else {
                TrbCompletionCode::TrbError
            }
        };
        CommandRingTrbHandler::command_completion_callback(&self.interrupter,
                                                           completion_code,
                                                           slot_id, atrb.gpa, &event_fd);
    }

    fn configure_endpoint(&self, atrb: &AddressedTrb, event_fd: EventFd) {
        debug!("configuring endpoint");
        let trb = atrb.trb.cast::<ConfigureEndpointCommandTrb>();
        let slot_id = trb.get_slot_id();
        let completion_code = {
            if valid_slot_id(slot_id) {
                self.slot(slot_id).configure_endpoint(trb)
            } else {
                TrbCompletionCode::TrbError
            }
        };
        CommandRingTrbHandler::command_completion_callback(&self.interrupter,
                                         completion_code, slot_id, atrb.gpa, &event_fd);
    }

    fn evaluate_context(&self, atrb: &AddressedTrb, event_fd: EventFd) {
        debug!("evaluating context");
        let trb = atrb.trb.cast::<EvaluateContextCommandTrb>();
        let slot_id = trb.get_slot_id();
        let completion_code = {
            if valid_slot_id(slot_id) {
                debug!("evaluating context for slot: {}", slot_id);
                self.slot(slot_id).evaluate_context(trb)
            } else {
                TrbCompletionCode::TrbError
            }
        };
        CommandRingTrbHandler::command_completion_callback(&self.interrupter,
                                                           completion_code,
                                                           slot_id, atrb.gpa, &event_fd);
    }

    fn reset_device(&self, atrb: &AddressedTrb, event_fd: EventFd) {
        debug!("reseting device");
        let trb = atrb.trb.cast::<ResetDeviceCommandTrb>();
        let slot_id = trb.get_slot_id();
        if valid_slot_id(slot_id) {
            let gpa = atrb.gpa;
            let interrupter = self.interrupter.clone();
            self.slots.reset_slot(slot_id, move |completion_code| {
                CommandRingTrbHandler::command_completion_callback(&interrupter,
                                                 completion_code,
                                                 slot_id, gpa, &event_fd);

            });
        } else {
            CommandRingTrbHandler::command_completion_callback(&self.interrupter,
                                                               TrbCompletionCode::TrbError,
                                                               slot_id, atrb.gpa, &event_fd);
        }
    }

    fn reset_endpoint(&self, atrb: &AddressedTrb, event_fd: EventFd) {
        debug!("reseting endpoint");
        let trb = atrb.trb.cast::<ResetEndpointCommandTrb>();
        let slot_id = trb.get_slot_id();
        let endpoint_id = trb.get_endpoint_id();
        error!("getting reset endpoint for slot {}, ep {}, linux driver only issue this when cmd ring stall. It should not happen here."
            ,slot_id, endpoint_id);
        CommandRingTrbHandler::command_completion_callback(&self.interrupter,
            TrbCompletionCode::Success, slot_id, atrb.gpa, &event_fd);
    }

    fn stop_endpoint(&self, atrb: &AddressedTrb, event_fd: EventFd) {
        debug!("stop endpoint");
        let trb = atrb.trb.cast::<StopEndpointCommandTrb>();
        let slot_id = trb.get_slot_id();
        let endpoint_id = trb.get_endpoint_id();
        if valid_slot_id(slot_id) {
            let gpa = atrb.gpa;
            let interrupter = self.interrupter.clone();
            self.slot(slot_id).stop_endpoint(endpoint_id, move |completion_code| {
                CommandRingTrbHandler::command_completion_callback(&interrupter,
                                                                   completion_code,
                                                                   slot_id, gpa, &event_fd);

            });
        } else {
            error!("stop endpoint trb has invalid slot id {}", slot_id);
            CommandRingTrbHandler::command_completion_callback(&self.interrupter,
                TrbCompletionCode::TrbError, slot_id, atrb.gpa, &event_fd);
        }
    }
    fn set_tr_dequeue_ptr(&self, atrb: &AddressedTrb, event_fd: EventFd) {
        debug!("stop endpoint");
        let trb = atrb.trb.cast::<SetTRDequeuePointerCommandTrb>();
        let slot_id = trb.get_slot_id();
        let endpoint_id = trb.get_trb_type();
        // See Set TR Dequeue Pointer Trb in spec.
        let dequeue_ptr = trb.get_dequeue_ptr() << 4;
        let completion_code = {
            if valid_slot_id(slot_id) {
                self.slot(slot_id).set_tr_dequeue_ptr(endpoint_id, dequeue_ptr)
            } else {
                error!("stop endpoint trb has invalid slot id {}", slot_id);
                TrbCompletionCode::TrbError
            }
        };
        CommandRingTrbHandler::command_completion_callback(
            &self.interrupter,
            TrbCompletionCode::TrbError,
            slot_id, atrb.gpa, &event_fd);
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
                CommandRingTrbHandler::command_completion_callback(&self.interrupter,
                                                                   TrbCompletionCode::Success, 0,
                                                                   atrb.gpa, &complete_event);
            },
            Some(TrbType::ResetEndpointCommand) => {
                error!("Receiving reset endpoint command. \
                       It should only happend when cmd ring stall");
                CommandRingTrbHandler::command_completion_callback(&self.interrupter, TrbCompletionCode::TrbError, 0,
                                                                   atrb.gpa, &complete_event);
            },
            Some(TrbType::StopEndpointCommand) =>
                self.stop_endpoint(atrb, complete_event),
            Some(TrbType::SetTRDequeuePointerCommand) =>
                self.set_tr_dequeue_ptr(atrb, complete_event),
            _ => {
                warn!(
                    // We are not handling type 14,15,16. See table 6.4.6.
                    "Unexpected command ring trb type: {}",
                    atrb.trb.get_trb_type()
                    );
                self.interrupter
                    .lock()
                    .unwrap()
                    .send_command_completion_trb(
                        TrbCompletionCode::TrbError,
                        0,
                        GuestAddress(atrb.gpa),
                        );
                complete_event.write(1).unwrap();

            },
        }
    }
}
