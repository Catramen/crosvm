// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex}
use xhci_regs::*;
use sys_util::{GuestAddress, GuestMemory};

use pci::pci_configuration::{
    PciClassCode, PciConfiguration, PciHeaderType, PciSerialBusSubClass
};

/// xHCI controller implementation.
pub struct Xhci {
    regs: XHCIRegs,
    command_ring_controller: CommandRingController,
}

impl Xhci {
    pub fn new(mem: GuestMemory, regs: XHCIRegs) -> Arc<Self> {
        let xhci = Arc::new(
            Xhci {
                regs,
            }
            );

    }

    pub fn reset() {
    }

    pub fn usbcmd_callback(&self, value: u32) {
        if value & USB_CMD_RESET {
            self.regs.usbsts.set_bits(USB_STS_CONTROLLER_NOT_READY);
            self.reset();
        }

        if value & USB_CMD_RUNSTOP {
            self.regs.usbsts.clear_bits(USB_STS_HALTED);
        } else {
            self.stop_all_transfer_ring_and_set_halted();
            self.crcr.clear_bits(CRCR_COMMAND_RING_RUNNING);
        }

        for i in 0... self.max_interrupters() {
            bool enable = value & USB_CMD_INTERRUPTER_ENABLE &&
                self.regs.iman[i].get_value & IMAN_INTERRUPT_ENABLE;
            self.interrupter(i).set_enabled(enabled);
        }
    }

    pub fn crcr_callback(&self, value: u64) {
        if !(self.regs.crcr.get_value() &  CRCR_COMMAND_RING_RUNNING {
            self.command_ring_controller.set_dequeue_pointer(
                GuestAddress(value & CRCR_COMMAND_RING_POINTER)
                );
            self.command_ring_controller.set_consumer_cycle_state(value & CRCR_RING_CYCLE_STATE);
        }
    }

    pub fn portsc_callback(&self, value: u32) {
    }

    pub fn doorbell_callback(&self, value: u32) {
    }

    pub fn iman_callback(&self, interrupter_index: i32, value: u32) {
    }

    pub fn imod_callback(&self, interrupter_index: i32, value: u32) {
    }

    pub fn erstsz_callback(&self, interrupter_index: i32, value: u32) {
    }

    pub fn erstba_callback(&self, interrupter_index: i32, value: u32) {
    }

    pub fn erdp_callback(&self, interrupter_index: i32, value: u32) {
    }


}
