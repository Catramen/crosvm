// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::interrupter::Interrupter;
use std::sync::{Arc, Mutex, Weak};
use sys_util::{EventFd, GuestAddress, GuestMemory};
use usb::auto_callback::AutoCallback;
use usb::event_loop::EventLoop;
use usb::xhci::command_ring_controller::CommandRingController;
use usb::xhci::device_slot::{DeviceSlot, DeviceSlots};
use usb::xhci::usb_hub::UsbHub;
use usb::xhci::xhci_abi::Trb;
use usb::xhci::xhci_regs::*;
use usb::xhci::xhci_backend_device_provider::XhciBackendDeviceProvider;
use usb::host_backend::host_backend_device_provider::HostBackendDeviceProvider;

/// xHCI controller implementation.
pub struct Xhci {
    mem: GuestMemory,
    regs: XHCIRegs,
    interrupter: Arc<Mutex<Interrupter>>,
    command_ring_controller: Arc<CommandRingController>,
    device_slots: DeviceSlots,
    device_provider: HostBackendDeviceProvider,
}

impl Xhci {
    /// Create a new xHCI controller.
    pub fn new(mem: GuestMemory, device_provider: HostBackendDeviceProvider,
               irq_evt: EventFd, regs: XHCIRegs) -> Arc<Self> {
        let (event_loop, _join_handle) = EventLoop::start();
        let interrupter = Arc::new(Mutex::new(Interrupter::new(mem.clone(), irq_evt, &regs)));
        let hub = Arc::new(UsbHub::new(&regs, interrupter.clone()));

        let mut device_provider = device_provider;
        device_provider.start(event_loop.clone(), hub.clone());

        let device_slots = DeviceSlots::new(
            regs.dcbaap.clone(),
            hub.clone(),
            interrupter.clone(),
            event_loop.clone(),
            mem.clone(),
        );
        let command_ring_controller = CommandRingController::new(
            mem.clone(),
            event_loop.clone(),
            device_slots.clone(),
            interrupter.clone(),
        );
        let xhci = Arc::new(Xhci {
            mem: mem.clone(),
            regs: regs,
            interrupter: interrupter,
            command_ring_controller: command_ring_controller,
            device_slots: device_slots,
            device_provider,
        });
        Self::init_reg_callbacks(&xhci);
        xhci
    }

    fn init_reg_callbacks(xhci: &Arc<Xhci>) {
        let xhci_weak = Arc::downgrade(xhci);
        xhci.regs.usbcmd.set_write_cb(move |val: u32| {
            xhci_weak.upgrade().unwrap().usbcmd_callback(val)
        });

        let xhci_weak = Arc::downgrade(xhci);
        xhci.regs
            .crcr
            .set_write_cb(move |val: u64| xhci_weak.upgrade().unwrap().crcr_callback(val));

        for i in 0..xhci.regs.portsc.len() {
            let xhci_weak = Arc::downgrade(xhci);
            xhci.regs.portsc[i].set_write_cb(move |val: u32| {
                xhci_weak.upgrade().unwrap().portsc_callback(i as u32, val)
            });
        }

        for i in 0..xhci.regs.doorbells.len() {
            let xhci_weak = Arc::downgrade(xhci);
            xhci.regs.doorbells[i].set_write_cb(move |val: u32| {
                xhci_weak
                    .upgrade()
                    .unwrap()
                    .doorbell_callback(i as u32, val);
                val
            });
        }

        let xhci_weak = Arc::downgrade(xhci);
        xhci.regs.iman.set_write_cb(move |val: u32| {
            xhci_weak.upgrade().unwrap().iman_callback(val);
            val
        });

        let xhci_weak = Arc::downgrade(xhci);
        xhci.regs.imod.set_write_cb(move |val: u32| {
            xhci_weak.upgrade().unwrap().imod_callback(val);
            val
        });

        let xhci_weak = Arc::downgrade(xhci);
        xhci.regs.erstsz.set_write_cb(move |val: u32| {
            xhci_weak.upgrade().unwrap().erstsz_callback(val);
            val
        });

        let xhci_weak = Arc::downgrade(xhci);
        xhci.regs.erstba.set_write_cb(move |val: u64| {
            xhci_weak.upgrade().unwrap().erstba_callback(val);
            val
        });

        let xhci_weak = Arc::downgrade(xhci);
        xhci.regs.erdp.set_write_cb(move |val: u64| {
            xhci_weak.upgrade().unwrap().erdp_callback(val);
            val
        });
    }

    // Callback for usbcmd register write.
    fn usbcmd_callback(&self, value: u32) -> u32 {
        if (value & USB_CMD_RESET) > 0 {
            debug!("xhci_controller: reset controller");
            self.reset();
            return value & (!USB_CMD_RESET);
        }

        if (value & USB_CMD_RUNSTOP) > 0 {
            debug!("xhci_controller: clear halt bits");
            self.regs.usbsts.clear_bits(USB_STS_HALTED);
        } else {
            debug!("xhci_controller: halt device");
            self.halt();
            self.regs.crcr.clear_bits(CRCR_COMMAND_RING_RUNNING);
        }

        // Enable interrupter if needed.
        let enabled = (value & USB_CMD_INTERRUPTER_ENABLE) > 0
            && (self.regs.iman.get_value() & IMAN_INTERRUPT_ENABLE) > 0;
        debug!("xhci_controller: interrupter enable?: {}", enabled);
        self.interrupter.lock().unwrap().set_enabled(enabled);
        value
    }

    // Callback for crcr register write.
    fn crcr_callback(&self, value: u64) -> u64 {
        debug!("xhci_controller: write to crcr {:x}", value);
        if (self.regs.crcr.get_value() & CRCR_COMMAND_RING_RUNNING) == 0 {
            self.command_ring_controller
                .set_dequeue_pointer(GuestAddress(value & CRCR_COMMAND_RING_POINTER));
            self.command_ring_controller
                .set_consumer_cycle_state((value & CRCR_RING_CYCLE_STATE) > 0);
            value
        } else {
            error!("Write to crcr while command ring is running");
            self.regs.crcr.get_value()
        }
    }

    // Callback for portsc register write.
    fn portsc_callback(&self, index: u32, value: u32) -> u32 {
        let mut value = value;
        debug!("xhci_controller: write to portsc index {} value {:x}", index, value);
        // xHCI spec 4.19.5. Note: we might want to change this logic if we support USB 3.0.
        if (value & PORTSC_PORT_RESET) > 0 || (value & PORTSC_WARM_PORT_RESET) > 0 {
            // Libusb onlys support blocking call to reset and "usually incurs a noticeable
            // delay.". We are faking a reset now.
            value &= !PORTSC_PORT_LINK_STATE_MASK;
            value &= !PORTSC_PORT_RESET;
            value |= PORTSC_PORT_ENABLED;
            value |= PORTSC_PORT_RESET_CHANGE;
            self.interrupter
                .lock()
                .unwrap()
                .send_port_status_change_trb((index + 1) as u8);
        }
        value
    }

    // Callback for doorbell register write.
    fn doorbell_callback(&self, index: u32, value: u32) {
        debug!("xhci_controller: write to doorbell index {} value {:x}", index, value);
        let target: usize = (value & DOORBELL_TARGET) as usize;
        let stream_id: u16 = (value >> DOORBELL_STREAM_ID_OFFSET) as u16;
        if (self.regs.usbcmd.get_value() & USB_CMD_RUNSTOP) > 0 {
            // First doorbell is for command ring.
            if index == 0 {
                if target != 0 || stream_id != 0 {
                    return;
                }
                debug!("doorbell to command ring");
                self.regs.crcr.set_bits(CRCR_COMMAND_RING_RUNNING);
                self.command_ring_controller.start();
            } else {
                debug!("doorbell to device slot");
                self.device_slots
                    .slot(index as u8)
                    .unwrap()
                    .ring_doorbell(target, stream_id);
            }
        }
    }

    // Callback for iman register write.
    fn iman_callback(&self, value: u32) {
        debug!("xhci_controller: write to iman {:x}", value);
        let enabled: bool = ((value & IMAN_INTERRUPT_ENABLE) > 0)
            && ((self.regs.usbcmd.get_value() & USB_CMD_INTERRUPTER_ENABLE) > 0);
        self.interrupter.lock().unwrap().set_enabled(enabled);
    }

    // Callback for imod register write.
    fn imod_callback(&self, value: u32) {
        debug!("xhci_controller: write to imod {:x}", value);
        self.interrupter.lock().unwrap().set_moderation(
            (value & IMOD_INTERRUPT_MODERATION_INTERVAL) as u16,
            (value >> IMOD_INTERRUPT_MODERATION_COUNTER_OFFSET) as u16,
        );
    }

    // Callback for erstsz register write.
    fn erstsz_callback(&self, value: u32) {
        debug!("xhci_controller: write to erstz {:x}", value);
        self.interrupter
            .lock()
            .unwrap()
            .set_event_ring_seg_table_size((value & ERSTSZ_SEGMENT_TABLE_SIZE) as u16);
    }

    // Callback for erstba register write.
    fn erstba_callback(&self, value: u64) {
        debug!("xhci_controller: write to erstba {:x}", value);
        self.interrupter
            .lock()
            .unwrap()
            .set_event_ring_seg_table_base_addr(GuestAddress(
                value & ERSTBA_SEGMENT_TABLE_BASE_ADDRESS,
            ));
    }

    // Callback for erdp register write.
    fn erdp_callback(&self, value: u64) {
        debug!("xhci_controller: write to erdp {:x}", value);
        {
            let mut interrupter = self.interrupter.lock().unwrap();
            interrupter.set_event_ring_dequeue_pointer(GuestAddress(
                value & ERDP_EVENT_RING_DEQUEUE_POINTER,
            ));
            interrupter.set_event_handler_busy((value & ERDP_EVENT_HANDLER_BUSY) > 0);
        }
    }

    fn reset(&self) {
        self.regs.usbsts.set_bits(USB_STS_CONTROLLER_NOT_READY);
        let usbsts = self.regs.usbsts.clone();
        self.device_slots.stop_all_and_reset(move || {
            usbsts.clear_bits(USB_STS_CONTROLLER_NOT_READY);
        });
    }

    fn halt(&self) {
        let usbsts = self.regs.usbsts.clone();
        self.device_slots.stop_all(AutoCallback::new(move || {
            usbsts.set_bits(USB_STS_HALTED);
        }));
    }
}
