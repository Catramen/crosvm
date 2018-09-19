// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::interrupter::Interrupter;
use super::mmio_register::Register;
use super::xhci_backend_device::XhciBackendDevice;
use super::xhci_regs::{
    XHCIRegs, MAX_PORTS, PORTSC_CONNECT_STATUS_CHANGE, PORTSC_CURRENT_CONNECT_STATUS,
    PORTSC_PORT_ENABLED, PORTSC_PORT_ENABLED_DISABLED_CHANGE, USB_STS_PORT_CHANGE_DETECT,
};
use std::sync::{Arc, Mutex};

pub struct UsbPorts {
    portsc: Vec<Register<u32>>,
    usbsts: Register<u32>,
    devices: Vec<Option<Arc<Mutex<XhciBackendDevice>>>>,
    interrupter: Arc<Mutex<Interrupter>>,
}

impl UsbPorts {
    pub fn new(regs: &XHCIRegs, interrupter: Arc<Mutex<Interrupter>>) -> UsbPorts {
        let mut vec = Vec::new();
        for i in 0..MAX_PORTS {
            vec.push(None);
        }
        UsbPorts {
            portsc: regs.portsc.clone(),
            usbsts: regs.usbsts.clone(),
            devices: vec,
            interrupter: interrupter,
        }
    }

    // Reset ports.
    pub fn reset(&self) {
        for i in 0..self.devices.len() {
            if self.devices[i].is_some() {
                let port_id = (i + 1) as u8;
                self.send_device_disconnected_event(port_id);
            }
        }
    }

    pub fn get_backend_for_port(&self, port_id: u8) -> Option<Arc<Mutex<XhciBackendDevice>>> {
        if port_id == 0 || port_id > MAX_PORTS {
            return None;
        }
        self.devices[(port_id - 1) as usize].clone()
    }

    pub fn connect_backend(&mut self, backend: Arc<Mutex<XhciBackendDevice>>) -> Option<u8> {
        for i in 0..self.devices.len() {
            if self.devices[i].is_none() {
                let port_id = (i + 1) as u8;
                self.devices[i] = Some(backend);
                self.send_device_connected_event(port_id);
                return Some(port_id);
            }
        }
        None
    }

    /// Inform the guest kernel there is device connected to this port. It combines first few steps
    /// of USB device initialization process in xHCI spec 4.3.
    pub fn send_device_connected_event(&self, port_id: u8) {
        if port_id == 0 || port_id > MAX_PORTS {
            return;
        }
        // xHCI spec 4.3.
        self.portsc[(port_id - 1) as usize].set_bits(
            PORTSC_CURRENT_CONNECT_STATUS
                | PORTSC_PORT_ENABLED
                | PORTSC_CONNECT_STATUS_CHANGE
                | PORTSC_PORT_ENABLED_DISABLED_CHANGE,
        );
        self.usbsts.set_bits(USB_STS_PORT_CHANGE_DETECT);
        self.interrupter
            .lock()
            .unwrap()
            .send_port_status_change_trb(port_id);
    }

    pub fn disconnect_backend(&mut self, port_id: u8) -> Result<(), ()> {
        if port_id == 0 || port_id > MAX_PORTS {
            return Err(());
        }
        if self.devices[(port_id - 1) as usize].is_none() {
            return Err(());
        }

        self.devices[(port_id - 1) as usize] = None;
        self.send_device_disconnected_event(port_id);
        Ok(())
    }

    pub fn send_device_disconnected_event(&self, port_id: u8) {
        if port_id == 0 || port_id > MAX_PORTS {
            return;
        }
        // xHCI spec 4.3.
        let index = (port_id - 1) as usize;
        self.portsc[index]
            .set_bits(PORTSC_CONNECT_STATUS_CHANGE | PORTSC_PORT_ENABLED_DISABLED_CHANGE);
        self.portsc[index].clear_bits(PORTSC_CURRENT_CONNECT_STATUS);
        self.usbsts.set_bits(USB_STS_PORT_CHANGE_DETECT);
        self.interrupter
            .lock()
            .unwrap()
            .send_port_status_change_trb(port_id);
    }
}
