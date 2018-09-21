// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use pci::{
    PciClassCode, PciConfiguration, PciDevice, PciDeviceError, PciHeaderType, PciInterruptPin,
    PciSerialBusSubClass,
};
use resources::SystemAllocator;
use std::os::unix::io::RawFd;
use std::sync::{Arc, Mutex};
use sys_util::{EventFd, GuestAddress, GuestMemory};
use usb::xhci::mmio_space::MMIOSpace;
use usb::xhci::xhci::Xhci;
use usb::xhci::xhci_regs::{init_xhci_mmio_space_and_regs, XHCIRegs};

const XHCI_BAR0_SIZE: u64 = 0x10000;

/// xHCI PCI interface implementation.
pub struct XhciController {
    config_regs: PciConfiguration,
    irq_evt: Option<EventFd>,
    mmio: Option<MMIOSpace>,
    mem: Option<GuestMemory>,
    xhci: Option<Arc<Xhci>>,
}

impl XhciController {
    pub fn new() -> Self {
        let mut config_regs = PciConfiguration::new(
            0x01b73, // fresco logic, (google = 0x1ae0)
            0x1000,  // fresco logic pdk. This chip has broken msi. See kernel xhci-pci.c
            PciClassCode::SerialBusController,
            &PciSerialBusSubClass::USB,
            PciHeaderType::Device,
            0,
            0,
        );
        XhciController {
            config_regs,
            irq_evt: None,
            mmio: None,
            mem: None,
            xhci: None,
        }
    }

    pub fn init_when_forked(&mut self) {
        let (mmio, regs) = init_xhci_mmio_space_and_regs();
        self.mmio = Some(mmio);
        self.xhci = Some(Xhci::new(
            self.mem.as_ref().unwrap().clone(),
            self.irq_evt.take().unwrap(),
            regs,
        ));
    }
}

impl PciDevice for XhciController {
    fn keep_fds(&self) -> Vec<RawFd> {
        Vec::new()
    }
    fn assign_irq(&mut self, irq_evt: EventFd, irq_num: u32, irq_pin: PciInterruptPin) {
        self.config_regs.set_irq(irq_num as u8, irq_pin);
        self.irq_evt = Some(irq_evt);
        debug!("xhci_controller: assign irq");
    }
    fn set_guest_memory(&mut self, mem: GuestMemory) {
        debug!("xhci_controller: set guest memory");
        self.mem = Some(mem);
    }
    fn allocate_io_bars(
        &mut self,
        resources: &mut SystemAllocator,
    ) -> Result<Vec<(u64, u64)>, PciDeviceError> {
        debug!("xhci_controller: Allocate io bars {}", XHCI_BAR0_SIZE);
        // xHCI spec 5.2.1.
        let bar0 = resources
            .allocate_mmio_addresses(XHCI_BAR0_SIZE)
            .ok_or(PciDeviceError::IoAllocationFailed(XHCI_BAR0_SIZE))?;
        self.config_regs
            .add_memory_region(bar0, XHCI_BAR0_SIZE)
            .ok_or(PciDeviceError::IoRegistrationFailed(bar0))?;
        Ok(vec![(bar0, XHCI_BAR0_SIZE)])
    }

    fn config_registers(&self) -> &PciConfiguration {
        &self.config_regs
    }

    fn config_registers_mut(&mut self) -> &mut PciConfiguration {
        debug!("xhci_controller: Config Register");
        &mut self.config_regs
    }

    fn read_bar(&mut self, addr: u64, data: &mut [u8]) {
        let bar0 = self.config_regs.get_bar_addr(0) as u64;
        if addr < bar0 || addr > bar0 + XHCI_BAR0_SIZE {
            return;
        }
        self.mmio.as_ref().unwrap().read_bar(addr - bar0, data);
        debug!("xhci_controller: read_bar addr: {}, data{:?}", addr, data);
    }

    fn write_bar(&mut self, addr: u64, data: &[u8]) {
        let bar0 = self.config_regs.get_bar_addr(0) as u64;
        if addr < bar0 || addr > bar0 + XHCI_BAR0_SIZE {
            return;
        }
        self.mmio.as_ref().unwrap().write_bar(addr - bar0, data);
        debug!(
            "xhci_controller: write_bar addr: {}, data: {:?}",
            addr, data
        );
    }
    fn on_sandboxed(&mut self) {
        self.init_when_forked();
    }
}
