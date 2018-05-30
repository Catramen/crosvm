// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use BusDevice;

use pci::pci_configuration::{PciClassCode, PciConfiguration, PciHeaderType, PciMultimediaSubclass};
use pci::pci_device::PciDevice;
use pci::pci_types::PciInterruptPin;
use sys_util::EventFd;

// TODO(jkwang) Move all pci ids to unified place.
const PCI_VENDOR_ID_REDHAT: u16 = 0x1036;
const PCI_DEVICE_ID_REDHAT_XHCI: u16 = 0x000d;

pub struct XHCIDev {
    config_regs: PciConfiguration,
}

impl XHCIDev {
    pub fn new() -> Self {
        let mut config_regs = PciConfiguration::new(PCI_VENDOR_ID_REDHAT,
                                                    PCI_DEVICE_ID_REDHAT_XHCI,
                                                    PciClassCode::SerialBusController,
                                                    // TODO(jkwang) set class, sub class.
                                                    // Programming interface.
                                                    &PciSerialBusSubclass::USBDevice,
                                                    PciHeaderType::Device);

        // TODO(jkwang)Set irq/MSI?.
        XHCIDev {
            config_regs,
        }
    }
}

impl PciDevice for XHCIDev {
    fn allocate_io_bars(
        &mut self,
        mut allocate: impl FnMut(u64) -> Option<u64>,
    ) -> Result<Vec<(u64, u64)>>
    where
        Self: Sized,
    {
        let mut ranges = Vec::new();
        let mmio_space = allocate(MIXER_REGS_SIZE)
            .ok_or(pci_device::Error::IoAllocationFailed(MIXER_REGS_SIZE))?;
        self.config_regs
            .add_io_region(mixer_regs_addr, MIXER_REGS_SIZE)
            .ok_or(pci_device::Error::IoRegistrationFailed(mixer_regs_addr))?;
        ranges.push((mixer_regs_addr, MIXER_REGS_SIZE));
        Ok(ranges)
    }

    // TODO(jkwang) fn in bar region. should be implemented in config_regs.

    fn config_registers(&self) -> &PciConfiguration {
        &self.config_regs
    }

    fn config_registers(&mut self) -> &mut PciConfiguration {
        &mut self.config_regs
    }

    fn read_bar(&mut self, addr: u64, data: &mut [u8]) {
    }

    fn write_bar(&mut self, addr: u64, data: &[u8]) {
    }
}

