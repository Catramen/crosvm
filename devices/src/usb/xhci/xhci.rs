// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::interrupter::Interrupter;
use std::sync::{Arc, Mutex, Weak};
use sys_util::{EventFd, GuestAddress, GuestMemory};
use usb::xhci::xhci_abi::Trb;
use usb::xhci::xhci_regs::*;

/// xHCI controller implementation.
pub struct Xhci {
    mem: GuestMemory,
    regs: XHCIRegs,
    interrupter: Arc<Mutex<Interrupter>>,
    // TODO(jkwang) Add command ring and device slot.
    // command_ring_controller: CommandRingController,
    // device_slot: [DeviceSlot; 8],
}

impl Xhci {
    /// Create a new xHCI controller.
    pub fn new(mem: GuestMemory, regs: XHCIRegs) -> Arc<Self> {
        let interrupter = Arc::new(Mutex::new(Interrupter::new(mem.clone(), &regs)));
        let xhci = Arc::new(Xhci {
            mem: mem.clone(),
            regs: regs,
            interrupter: interrupter,
        });
        Self::init_reg_callbacks(&xhci);
        xhci
    }

    fn init_reg_callbacks(xhci: &Arc<Xhci>) {
        let xhci_weak = Arc::downgrade(xhci);

        let xhci_weak0 = xhci_weak.clone();
        xhci.regs.usbcmd.set_write_cb(move |val: u32| {
            xhci_weak0.upgrade().unwrap().usbcmd_callback(val);
            val
        });

        let xhci_weak0 = xhci_weak.clone();
        xhci.regs.crcr.set_write_cb(move |val: u64| {
            xhci_weak0.upgrade().unwrap().crcr_callback(val);
            val
        });

        for i in 0..xhci.regs.portsc.len() {
            let xhci_weak0 = xhci_weak.clone();
            xhci.regs.portsc[i].set_write_cb(move |val: u32| {
                xhci_weak0.upgrade().unwrap().portsc_callback(i as u32, val);
                val
            });
        }

        for i in 0..xhci.regs.doorbells.len() {
            let xhci_weak0 = xhci_weak.clone();
            xhci.regs.doorbells[i].set_write_cb(move |val: u32| {
                xhci_weak0
                    .upgrade()
                    .unwrap()
                    .doorbell_callback(i as u32, val);
                val
            });
        }

        let xhci_weak0 = xhci_weak.clone();
        xhci.regs.iman.set_write_cb(move |val: u32| {
            xhci_weak0.upgrade().unwrap().iman_callback(val);
            val
        });

        let xhci_weak0 = xhci_weak.clone();
        xhci.regs.imod.set_write_cb(move |val: u32| {
            xhci_weak0.upgrade().unwrap().imod_callback(val);
            val
        });

        let xhci_weak0 = xhci_weak.clone();
        xhci.regs.erstsz.set_write_cb(move |val: u32| {
            xhci_weak0.upgrade().unwrap().erstsz_callback(val);
            val
        });

        let xhci_weak0 = xhci_weak.clone();
        xhci.regs.erstba.set_write_cb(move |val: u64| {
            xhci_weak0.upgrade().unwrap().erstba_callback(val);
            val
        });

        let xhci_weak0 = xhci_weak.clone();
        xhci.regs.erdp.set_write_cb(move |val: u64| {
            xhci_weak0.upgrade().unwrap().erdp_callback(val);
            val
        });
    }
    /// Get the guest memory.
    pub fn guest_mem(&self) -> &GuestMemory {
        &self.mem
    }

    /// Set the EventFd of legacy PCI IRQ.
    pub fn set_interrupt_fd(&self, fd: EventFd) {
        self.interrupter.lock().unwrap().set_interrupt_fd(fd);
    }

    pub fn send_event(&self, trb: Trb) {
        self.interrupter.lock().unwrap().add_event(trb);
    }

    // Callback for usbcmd register write.
    fn usbcmd_callback(&self, value: u32) {
        // Enable interrupter if needed.
        let enabled = (value & USB_CMD_INTERRUPTER_ENABLE) > 0
            && (self.regs.iman.get_value() & IMAN_INTERRUPT_ENABLE) > 0;
        self.interrupter.lock().unwrap().set_enabled(enabled);
    }

    // Callback for crcr register write.
    fn crcr_callback(&self, value: u64) {
        // TODO(jkwang) Implement side effects of crcr register write.
    }

    // Callback for portsc register write.
    fn portsc_callback(&self, index: u32, value: u32) {
        // TODO(jkwang) Implement side effects of portsc register write.
    }

    // Callback for doorbell register write.
    fn doorbell_callback(&self, index: u32, value: u32) {
        // TODO(jkwang) Implement side effects of doorbell register write.
    }

    // Callback for iman register write.
    fn iman_callback(&self, value: u32) {
        let enabled: bool = ((value & IMAN_INTERRUPT_ENABLE) > 0)
            && ((self.regs.usbcmd.get_value() & USB_CMD_INTERRUPTER_ENABLE) > 0);
        self.interrupter.lock().unwrap().set_enabled(enabled);
    }

    // Callback for imod register write.
    fn imod_callback(&self, value: u32) {
        self.interrupter.lock().unwrap().set_moderation(
            (value & IMOD_INTERRUPT_MODERATION_INTERVAL) as u16,
            (value >> IMOD_INTERRUPT_MODERATION_COUNTER_OFFSET) as u16,
        );
    }

    // Callback for erstsz register write.
    fn erstsz_callback(&self, value: u32) {
        self.interrupter
            .lock()
            .unwrap()
            .set_event_ring_seg_table_size((value & ERSTSZ_SEGMENT_TABLE_SIZE) as u16);
    }

    // Callback for erstba register write.
    fn erstba_callback(&self, value: u64) {
        self.interrupter
            .lock()
            .unwrap()
            .set_event_ring_seg_table_base_addr(GuestAddress(
                value & ERSTBA_SEGMENT_TABLE_BASE_ADDRESS,
            ));
    }

    // Callback for erdp register write.
    fn erdp_callback(&self, value: u64) {
        {
            let mut interrupter = self.interrupter.lock().unwrap();
            interrupter.set_event_ring_dequeue_pointer(GuestAddress(
                value & ERDP_EVENT_RING_DEQUEUE_POINTER,
            ));
            interrupter.set_event_handler_busy((value & ERDP_EVENT_HANDLER_BUSY) > 0);
        }
    }
}
