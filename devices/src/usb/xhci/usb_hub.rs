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
use std::sync::{Arc, Mutex, MutexGuard};
use std::ops::Deref;

/// Error type for usb ports.
pub enum Error {
    InvalidPort,
    PortEmpty,
}

/// A port on usb hub. It could have a device connected to it.
pub struct UsbPort {
    port_id: u8,
    portsc: Register<u32>,
    usbsts: Register<u32>,
    interrupter: Arc<Mutex<Interrupter>>,
    backend_device: Mutex<Option<Box<XhciBackendDevice>>>,
}

impl UsbPort {
    /// Create a new usb port that has nothing connected to it.
    pub fn new(port_id: u8, portsc: Register<u32>, usbsts: Register<u32>,
               interrupter: Arc<Mutex<Interrupter>>) -> UsbPort {
        UsbPort{
            port_id,
            portsc,
            usbsts,
            interrupter,
            backend_device: Mutex::new(None)
        }
    }

    /// Detach current connected backend.
    pub fn detach(&self) -> bool {
        let mut locked = self.backend_device.lock().unwrap();
        if locked.is_none() {
            error!("device is already detached from this port {}", self.port_id);
            return false;
        }
        debug!("device detached from port {}", self.port_id);
        *locked = None;
        self.send_device_disconnected_event();
        true
    }

    /// Get current connected backend.
    pub fn get_backend_device(&self) -> MutexGuard<Option<Box<XhciBackendDevice>>> {
        self.backend_device.lock().unwrap()
    }

    fn reset(&self) {
        if self.backend_device.lock().unwrap().is_some() {
            self.send_device_connected_event();
        }
    }

    fn attach(&self, device: Box<XhciBackendDevice>) {
        debug!("A backend is connected to port {}", self.port_id);
        let mut locked = self.backend_device.lock().unwrap();
        assert!(locked.is_none());
        *locked = Some(device);
        self.send_device_connected_event();
    }

    /// Inform the guest kernel there is device connected to this port. It combines first few steps
    /// of USB device initialization process in xHCI spec 4.3.
    pub fn send_device_connected_event(&self) {
        // xHCI spec 4.3.
        self.portsc.set_bits(
            PORTSC_CURRENT_CONNECT_STATUS
                | PORTSC_PORT_ENABLED
                | PORTSC_CONNECT_STATUS_CHANGE
                | PORTSC_PORT_ENABLED_DISABLED_CHANGE,
        );
        self.usbsts.set_bits(USB_STS_PORT_CHANGE_DETECT);
        self.interrupter
            .lock()
            .unwrap()
            .send_port_status_change_trb(self.port_id);
    }

    /// Inform the guest kernel that device has been detached.
    pub fn send_device_disconnected_event(&self) {
        // xHCI spec 4.3.
        self.portsc
            .set_bits(PORTSC_CONNECT_STATUS_CHANGE | PORTSC_PORT_ENABLED_DISABLED_CHANGE);
        self.portsc.clear_bits(PORTSC_CURRENT_CONNECT_STATUS);
        self.usbsts.set_bits(USB_STS_PORT_CHANGE_DETECT);
        self.interrupter
            .lock()
            .unwrap()
            .send_port_status_change_trb(self.port_id);
    }
}

/// UsbHub is a set of usb ports.
pub struct UsbHub {
    ports: Vec<Arc<UsbPort>>,
}

impl UsbHub {
    /// Create usb hub with no device attached.
    pub fn new(regs: &XHCIRegs, interrupter: Arc<Mutex<Interrupter>>) -> UsbHub {
        let mut vec = Vec::new();
        // Each port should have a portsc register.
        assert_eq!(MAX_PORTS as usize, regs.portsc.len());

        for i in 0..MAX_PORTS {
            vec.push(Arc::new(
                    UsbPort::new(i + 1, regs.portsc[i as usize].clone(),
                                regs.usbsts.clone(), interrupter.clone())
                    ));
        }
        UsbHub {
            ports: vec,
        }
    }

    /// Reset all ports.
    pub fn reset(&self) {
        debug!("reseting usb hub");
        for p in &self.ports {
            p.reset();
        }
    }

    /// Get a specific port of the hub.
    pub fn get_port(&self, port_id: u8) -> Option<Arc<UsbPort>> {
        if port_id == 0 || port_id > MAX_PORTS {
            return None;
        }
        Some(self.ports[(port_id - 1) as usize].clone())
    }

    /// Connect backend to next empty port.
    pub fn connect_backend(&self, backend: Box<XhciBackendDevice>) -> Option<u8> {
        debug!("Trying to connect backend to hub");
        for i in 0..self.ports.len() {
            if (*self.ports[i].get_backend_device()).is_none() {
                self.ports[i].attach(backend);
                return Some((i + 1) as u8);
            }
        }
        None
    }

    /// Disconnect device from port.
    pub fn disconnect_port(&self, port_id: u8) -> bool {
        if port_id == 0 || port_id > MAX_PORTS {
            return false;
        }
        self.ports[port_id  as usize - 1].detach()
    }
}
