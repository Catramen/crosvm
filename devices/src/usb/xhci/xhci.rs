// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex, Weak};
use sys_util::{GuestAddress, GuestMemory};
use usb::xhci::xhci_regs::XHCIRegs;

/// xHCI controller implementation.
pub struct Xhci {
    mem: GuestMemory,
    regs: XHCIRegs,
    // TODO(jkwang) Add command ring and device slot.
    // command_ring_controller: CommandRingController,
    // device_slot: [DeviceSlot; 8],
}

impl Xhci {
    /// Create a new xHCI controller.
    pub fn new(mem: GuestMemory, regs: XHCIRegs) -> Arc<Self> {
        let xhci = Arc::new(Xhci { mem, regs });
        let xhci_weak = Arc::downgrade(&xhci.clone());

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

        xhci
    }

    /// Get the guest memory.
    pub fn guest_mem(&self) -> &GuestMemory {
        &self.mem
    }

    // Callback for usbcmd register write.
    fn usbcmd_callback(&self, value: u32) {
        // TODO(jkwang) Implement side effects of usbcmd register write.
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
        // TODO(jkwang) Implement side effects of iman register write.
    }

    // Callback for imod register write.
    fn imod_callback(&self, value: u32) {
        // TODO(jkwang) Implement side effects of imod register write.
    }

    // Callback for erstsz register write.
    fn erstsz_callback(&self, value: u32) {
        // TODO(jkwang) Implement side effects of erstsz register write.
    }

    // Callback for erstba register write.
    fn erstba_callback(&self, value: u64) {
        // TODO(jkwang) Implement side effects of erstba register write.
    }

    // Callback for erdp register write.
    fn erdp_callback(&self, value: u64) {
        // TODO(jkwang) Implement side effects of erdp register write.
    }
}
