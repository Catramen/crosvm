// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::event_ring::EventRing;
use super::mmio_register::Register;
use super::xhci_abi::{
    AddressedTrb, CommandCompletionEventTrb, PortStatusChangeEventTrb, Trb, TrbCast,
    TrbCompletionCode, TrbType,
};
use super::xhci_regs::*;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::sync::{Arc, Mutex, Weak};
use sys_util::{EventFd, GuestAddress, GuestMemory};

/// See spec 4.17 for interrupters. Controller can send an event back to guest kernel driver
/// through interrupter.
pub struct Interrupter {
    interrupt_fd: Option<EventFd>,
    usbsts: Register<u32>,
    iman: Register<u32>,
    erdp: Register<u64>,
    event_handler_busy: bool,
    enabled: bool,
    pending: bool,
    moderation_interval: u16,
    moderation_counter: u16,
    event_ring: EventRing,
}

impl Interrupter {
    pub fn new(mem: GuestMemory, regs: &XHCIRegs) -> Self {
        Interrupter {
            interrupt_fd: None,
            usbsts: regs.usbsts.clone(),
            iman: regs.iman.clone(),
            erdp: regs.erdp.clone(),
            event_handler_busy: false,
            enabled: false,
            pending: false,
            moderation_interval: 0,
            moderation_counter: 0,
            event_ring: EventRing::new(mem),
        }
    }

    pub fn set_interrupt_fd(&mut self, fd: EventFd) {
        self.interrupt_fd = Some(fd);
    }

    pub fn add_event(&mut self, trb: Trb) {
        self.event_ring.add_event(trb);
        self.pending = true;
    }

    pub fn send_port_status_change_trb(&mut self, port_id: u8) {
        let mut trb = Trb::new();
        {
            let mut psctrb = trb.cast_mut::<PortStatusChangeEventTrb>();
            psctrb.set_port_id(port_id);
            psctrb.set_completion_code(TrbCompletionCode::Success as u8);
            psctrb.set_trb_type(TrbType::PortStatusChangeEvent as u8);
        }
        self.add_event(trb);
    }

    pub fn send_command_completion_trb(
        &mut self,
        completion_code: TrbCompletionCode,
        slot_id: u8,
        trb_addr: GuestAddress,
    ) {
        let mut trb = Trb::new();
        {
            let mut ctrb = trb.cast_mut::<CommandCompletionEventTrb>();
            ctrb.set_trb_pointer(trb_addr.0);
            ctrb.set_command_completion_parameter(0);
            ctrb.set_completion_code(completion_code as u8);
            ctrb.set_trb_type(TrbType::CommandCompletionEvent as u8);
            ctrb.set_vf_id(0);
            ctrb.set_slot_id(slot_id);
        }
        self.add_event(trb);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.interrupt_if_needed();
    }

    pub fn set_moderation(&mut self, interval: u16, counter: u16) {
        // TODO(jkwang) Moderation is not implemented yet.
        self.moderation_interval = interval;
        self.moderation_counter = counter;
        self.interrupt_if_needed();
    }

    pub fn set_event_ring_seg_table_size(&mut self, size: u16) {
        self.event_ring.set_seg_table_size(size);
    }

    pub fn set_event_ring_seg_table_base_addr(&mut self, addr: GuestAddress) {
        self.event_ring.set_seg_table_base_addr(addr);
    }

    pub fn set_event_ring_dequeue_pointer(&mut self, addr: GuestAddress) {
        self.event_ring.set_dequeue_pointer(addr);
        if addr == self.event_ring.get_enqueue_pointer() {
            self.pending = false;
        }
        self.interrupt_if_needed();
    }

    pub fn set_event_handler_busy(&mut self, busy: bool) {
        self.event_handler_busy = busy;
        self.interrupt_if_needed();
    }

    fn interrupt_if_needed(&mut self) {
        if self.enabled && self.pending && !self.event_handler_busy {
            self.event_handler_busy = true;
            self.pending = false;
            self.usbsts.set_bits(USB_STS_EVENT_INTERRUPT);
            self.iman.set_bits(IMAN_INTERRUPT_PENDING);
            self.erdp.set_bits(ERDP_EVENT_HANDLER_BUSY);
            self.interrupt_fd.as_ref().unwrap().write(1).unwrap();
        }
    }
}
