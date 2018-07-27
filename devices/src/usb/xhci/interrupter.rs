// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex, Weak};
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use super::EventRing;
use sys_util::{GuestAddress, GuestMemory};
use super::xhci_abi::{
    AddressedTrb,
    TrbCompletionCode,
};

pub struct Interrupter {
    imp: Mutex<InterrupterImpl>,
}

struct InterrupterImpl {
    // index of this interrupter.
    index: u8,
    event_handler_busy: bool,
    enabled: bool,
    paused: bool,
    pending: bool,
    moderation_interval: u16,
    moderation_counter: u16,
    xhci: Weak<Xhci>,
    event_ring: EventRing,
}

impl Interrupter {
    pub fn new() -> Self {
    }

    pub fn add_event(&self, trb: Trb) {
        let imp = self.imp.lock().unwrap();
        imp.event_ring.add_event(trb);
        imp.pending = true;
        imp.signal_interrupt_if_needed();
    }

    pub fn set_enabled(&self, enabled: bool) {
        let imp = self.imp.lock().unwrap();
        imp.enabled = enabled;
        imp.signal_interrupt_if_needed();
    }

    pub fn set_moderation(&self, interval: u16, counter: u16) {
        let imp = self.imp.lock().unwrap();
        // Moderation is not implemented yet.
        // TODO(jkwang) prev line.
        imp.moderation_interval = interval;
        imp.moderation_counter = counter;
        imp.signal_interrupt_if_needed();
    }

    pub fn set_event_ring_seg_table_size(&self, size: u16) {
        let imp = self.imp.lock().unwrap();
        imp.event_ring.set_seg_table_size(size);
    }

    pub fn set_event_ring_seg_table_base_addr(&self, addr: GuestAddress) {
        let imp = self.imp.lock().unwrap();
        imp.event_ring.set_seg_table_base_addr(addr);
    }

    pub fn set_event_ring_dequeue_pointer(&self, addr: GuestAddress) {
        let imp = self.imp.lock().unwrap();
        imp.event_ring.set_dequeue_pointer(addr);
        if addr == imp.event_ring.get_enqueue_pointer() {
            imp.pending = false;
        }
    }

    pub fn set_event_handler_busy(&self, busy: bool) {
        let imp = self.imp.lock().unwrap();
        imp.event_handler_busy = busy;
        imp.signal_interrupt_if_needed();
    }
}

impl InterrupterImpl {
    pub fn signal_interrupt_if_needed(&self) {
        if self.enabled && self.pending && !self.event_handler_busy && !self.paused {
            self.xhci.upgrade().unwrap().send_interrupt(self.index);
            self.event_handler_busy = true;
            self.pending = false;
        }
    }
}
