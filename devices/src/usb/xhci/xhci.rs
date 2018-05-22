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
                                                    &PciSerialBusSubclass::USBDevice,
                                                    PciHeaderType::Device);

        // TODO(jkwang)Set irq/MSI?.
        XHCIDev {
            config_regs,
        }
    }
}

impl PciDevice for XHCIDev {
    fn bar_region(&self, addr: u64) -> Option<(u64, Arc<Mutex<BusDevice>>)> {
        None
    }

    fn config_registers(&self) -> &PciConfiguration {
        &self.config_regs
    }

    fn config_registers_mut(&mut self) -> &mut PciConfiguration {
        &mut self.config_regs
    }
}

