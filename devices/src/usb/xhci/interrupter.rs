// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::event_ring::EventRing;
use super::mmio_register::Register;
use super::xhci_abi::{
    CommandCompletionEventTrb, PortStatusChangeEventTrb, TransferEventTrb, Trb, TrbCast,
    TrbCompletionCode, TrbType,
};
use super::xhci_regs::*;
use sys_util::{EventFd, GuestAddress, GuestMemory};
use usb::error::{Error, Result};

/// See spec 4.17 for interrupters. Controller can send an event back to guest kernel driver
/// through interrupter.
pub struct Interrupter {
    interrupt_fd: EventFd,
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
    /// Create a new interrupter.
    pub fn new(mem: GuestMemory, irq_evt: EventFd, regs: &XhciRegs) -> Self {
        Interrupter {
            interrupt_fd: irq_evt,
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

    /// Returns true if event ring is empty.
    pub fn event_ring_is_empty(&self) -> bool {
        self.event_ring.is_empty()
    }

    /// Add event to event ring.
    fn add_event(&mut self, trb: Trb) -> Result<()> {
        self.event_ring
            .add_event(trb)
            .map_err(err_msg!(Error::BadState))?;
        self.pending = true;
        self.interrupt_if_needed()
    }

    /// Send port status change trb for port.
    pub fn send_port_status_change_trb(&mut self, port_id: u8) -> Result<()> {
        let mut trb = Trb::new();
        {
            let psctrb = trb.cast_mut::<PortStatusChangeEventTrb>()?;
            psctrb.set_port_id(port_id);
            psctrb.set_completion_code(TrbCompletionCode::Success as u8);
            psctrb.set_trb_type(TrbType::PortStatusChangeEvent as u8);
        }
        self.add_event(trb)
    }

    /// Send command completion trb.
    pub fn send_command_completion_trb(
        &mut self,
        completion_code: TrbCompletionCode,
        slot_id: u8,
        trb_addr: GuestAddress,
    ) -> Result<()> {
        let mut trb = Trb::new();
        {
            let ctrb = trb.cast_mut::<CommandCompletionEventTrb>()?;
            ctrb.set_trb_pointer(trb_addr.0);
            ctrb.set_command_completion_parameter(0);
            ctrb.set_completion_code(completion_code as u8);
            ctrb.set_trb_type(TrbType::CommandCompletionEvent as u8);
            ctrb.set_vf_id(0);
            ctrb.set_slot_id(slot_id);
        }
        self.add_event(trb)
    }

    /// Send transfer event trb.
    pub fn send_transfer_event_trb(
        &mut self,
        completion_code: TrbCompletionCode,
        trb_pointer: u64,
        transfer_length: u32,
        event_data: bool,
        slot_id: u8,
        endpoint_id: u8,
    ) -> Result<()> {
        let mut trb = Trb::new();
        {
            let event_trb = trb.cast_mut::<TransferEventTrb>()?;
            event_trb.set_trb_pointer(trb_pointer);
            event_trb.set_trb_transfer_length(transfer_length);
            event_trb.set_completion_code(completion_code as u8);
            event_trb.set_event_data(event_data.into());
            event_trb.set_trb_type(TrbType::TransferEvent as u8);
            event_trb.set_endpoint_id(endpoint_id);
            event_trb.set_slot_id(slot_id);
        }
        self.add_event(trb)
    }

    /// Enable/Disable this interrupter.
    pub fn set_enabled(&mut self, enabled: bool) -> Result<()> {
        debug!("interrupter set enabled {}", enabled);
        self.enabled = enabled;
        self.interrupt_if_needed().map_err(err_msg!())
    }

    /// Set interrupt moderation.
    pub fn set_moderation(&mut self, interval: u16, counter: u16) -> Result<()> {
        // TODO(jkwang) Moderation is not implemented yet.
        self.moderation_interval = interval;
        self.moderation_counter = counter;
        self.interrupt_if_needed().map_err(err_msg!())
    }

    /// Set event ring seg table size.
    pub fn set_event_ring_seg_table_size(&mut self, size: u16) -> Result<()> {
        debug!("interrupter set seg table size {}", size);
        self.event_ring
            .set_seg_table_size(size)
            .map_err(err_msg!(Error::BadState))
    }

    /// Set event ring segment table base address.
    pub fn set_event_ring_seg_table_base_addr(&mut self, addr: GuestAddress) -> Result<()> {
        debug!("interrupter set table base addr {:#x}", addr.0);
        self.event_ring
            .set_seg_table_base_addr(addr)
            .map_err(err_msg!(Error::BadState))
    }

    /// Set event ring dequeue pointer.
    pub fn set_event_ring_dequeue_pointer(&mut self, addr: GuestAddress) -> Result<()> {
        debug!("interrupter set dequeue ptr addr {:#x}", addr.0);
        self.event_ring.set_dequeue_pointer(addr);
        if addr == self.event_ring.get_enqueue_pointer() {
            self.pending = false;
        }
        self.interrupt_if_needed().map_err(err_msg!())
    }

    /// Set event hander busy.
    pub fn set_event_handler_busy(&mut self, busy: bool) -> Result<()> {
        debug!("set event handler busy {}", busy);
        self.event_handler_busy = busy;
        self.interrupt_if_needed().map_err(err_msg!())
    }

    fn interrupt_if_needed(&mut self) -> Result<()> {
        if self.enabled && self.pending && !self.event_handler_busy {
            debug!("sending interrupt");
            self.event_handler_busy = true;
            self.pending = false;
            self.usbsts.set_bits(USB_STS_EVENT_INTERRUPT);
            self.iman.set_bits(IMAN_INTERRUPT_PENDING);
            self.erdp.set_bits(ERDP_EVENT_HANDLER_BUSY);
            self.interrupt_fd
                .write(1)
                .map_err(err_msg!(Error::SysError))?;
        }
        Ok(())
    }
}
