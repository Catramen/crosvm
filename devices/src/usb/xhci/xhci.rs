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
    mem: GuestMemory,
    regs: XHCIRegs,
    command_ring_controller: CommandRingController,
    device_slot: [DeviceSlot; 8],
}

impl Xhci {
    pub fn new(mem: GuestMemory, regs: XHCIRegs) -> Arc<Self> {
    }

    pub fn guest_mem(&self) -> &GuestMemory {
        self.mem
    }

    pub fn usbcmd_callback(&self, value: u32) {
        if value & USB_CMD_RESET {
            self.regs.usbsts.set_bits(USB_STS_CONTROLLER_NOT_READY);
            self.reset();
            return;
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
        if !(self.regs.crcr.get_value() &  CRCR_COMMAND_RING_RUNNING) {
            self.command_ring_controller.set_dequeue_pointer(
                GuestAddress(value & CRCR_COMMAND_RING_POINTER)
                );
            self.command_ring_controller.set_consumer_cycle_state(value & CRCR_RING_CYCLE_STATE);
        }
        // TODO(jkwang) should we ignore crcr when command ring is running.
    }

    pub fn portsc_callback(&self, index: u32, value: u32) {
        // xHCI spec section spec 4.19.5.
        backend().reset();
    }

    pub fn doorbell_callback(&self, index: u32, value: u32) {
        let target: u8 = value & DOORBELL_TARGET;
        let stream_id: u16 = value >> DOORBELL_STREAM_ID_OFFSET;
        if self.regs.usbcmd.get_value() & USB_CMD_RUNSTOP {
            // First doorbell is for command ring.
            if index == 0 {
                if target != 0 || stream_id != 0 {
                    return;
                }
                self.regs.crcr.set_bits(CRCR_COMMAND_RING_RUNNING);
                self.command_ring_controller.start();
            } else {
                self.device_slot(index).doorbell(target, stream_id);
            }
        }
    }

    pub fn iman_callback(&self, index: u32, value: u32) {
        let enabled: bool = (value & IMAN_INTERRUPT_ENABLE) &&
            (self.usbcmd.get_value() & USB_CMD_INTERRUPTER_ENABLE);
        self.interrupter(index).set_enabled(enabled);
    }

    pub fn imod_callback(&self, index: u32, value: u32) {
        self.interrupter(index).set_moderation(value & IMOD_INTERRUPT_MODERATION_INTERVAL,
                                               value >> IMOD_INTERRUPT_MODERATION_COUNTER_OFFSET);
    }

    pub fn erstsz_callback(&self, index: u32, value: u32) {
        self.interrupter(index).set_event_ring_seg_table_size(value & ERSTSZ_SEGMENT_TABLE_SIZE);
    }

    pub fn erstba_callback(&self, index: u32, value: u32) {
        self.interrupter(index).
            set_event_ring_seg_table_base_addr(
                GuestAddress(value & ERSTBA_SEGMENT_TABLE_BASE_ADDRESS));
    }

    pub fn erdp_callback(&self, index: u32, value: u32) {
        self.interrupter(index).
            set_event_ring_dequeue_pointer(
                GuestAddress(value & ERDP_EVENT_RING_DEQUEUE_POINTER);
                );
        self.interrupter(index).
            set_event_handler_busy(value & ERDP_EVENT_HANDLER_BUSY);
    }

    fn reset() {
    }
}

// Bitmasks and offsets for structural parameter registers.
const HCSPARAMS1_MAX_INTERRUPTERS_MASK: u32 = 0x7FF00;
const HCSPARAMS1_MAX_INTERRUPTERS_OFFSET: u32 = 8;
const HCSPARAMS1_MAX_SLOTS_MASK: u32 = 0xFF;

// Bitmasks and offsets for extended capabilities registers.
const SPCAP_PORT_COUNT_MASK: u32 = 0xFF00;
const SPCAP_PORT_COUNT_OFFSET: u32 = 8;
