// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex}
use xhci_regs::{XHCIRegs, init_xhci_mmio_space_and_regs, XHCI_BAR0_SIZE}
use pci::pci_configuration::{
    PciClassCode, PciConfiguration, PciHeaderType, PciSerialBusSubClass
};

/// xHCI controller implementation.
pub struct XhciDevice {
    config_regs: PciConfiguration,
    mmio: MMIOSpace,
    xhci: Arc<Xhci>,
}

impl XhciDevice {
    pub fn new(mem: GuestMemory) -> Self {
        let mut config_regs = PciConfiguration::new(
            0x01b73, // fresco logic, (google = 0x1ae0)
            0x1000, // fresco logic pdk. This chip has broken msi. See kernel xhci-pci.c
            PciClassCode::SerialBusController,
            &PciSerialBusSubClass::USB,
            header_type: PciHeaderType::Device,
            );
        let (mmio, regs) = init_xhci_mmio_space_and_regs();
        XhciDevice {
            config_regs,
            mmio,
        }
    }

}

impl PciDevice for XhciDevice {
    fn assign_irq(&mut self, irq_evt: EventFd, irq_num: u32, irq_pin: PciInterruptPin) {
    }

    fn allocate_io_bars(
        &mut self,
        resources: &mut SystemAllocator,
        ) -> Result<Vec<(u64, u64)>> {
        // xHCI spec 5.2.1.
        let bar0 = resources.allocate_mmio_addresses(XHCI_BAR0_SIZE)
            .ok_or(pci_device::Error::IoAllocationFailed(MIXER_REGS_SIZE))?;
        self.config_regs
            .add_memory_region(bar0, XHCI_BAR0_SIZE)
            .ok_or(pci_device::Error::IoRegistrationFailed(mixer_regs_addr))?;
        Ok(vec![])
    }

    fn config_registers(&self) -> &PciConfiguration {
        &self.config_regs
    }

    fn config_registers_mut(&mut self) -> &mut PciConfiguration {
        &mut self.config_regs
    }

    fn read_bar(&mut self, addr: u64, data: &mut [u8]) {
        let bar0 = self.config_regs.get_bar_addr(0) as u64;
        if addr < bar0 || addr > bar0 + XHCI_BAR0_SIZE {
            return ()
        }
        self.mmio.read_bar(addr - bar0, data);
    }

    fn write_bar(&mut self, addr: u64, data: &[u8]) {
        let bar0 = self.config_regs.get_bar_addr(0) as u64;
        if addr < bar0 || addr > bar0 + XHCI_BAR0_SIZE {
            return ()
        }
        self.mmio.write_bar(addr - bar0, data]);
    }
}
