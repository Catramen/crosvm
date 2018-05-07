// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use pci::pci_configuration::{PciClassCode, PciConfiguration, PciHeaderType, PciMultimediaSubclass};
use pci::pci_device::PciDevice;

// Use 82801AA because it's what qemu does.
const PCI_DEVICE_ID_INTEL_82801AA_5: u16 = 0x2415;

/// AC97 audio device emulation.
pub struct Ac97 {
    config_regs: PciConfiguration,
}

impl Ac97 {
    pub fn new() -> Self {
        let mut config_regs = PciConfiguration::new(0x8086,
                                                    PCI_DEVICE_ID_INTEL_82801AA_5,
                                                    PciClassCode::MultimediaController,
                                                    &PciMultimediaSubclass::AudioDevice,
                                                    PciHeaderType::Device);
        config_regs.add_io_region(0x0000_1000, 0x0000_0100).unwrap();
        config_regs.add_io_region(0x0000_1400, 0x0000_0400).unwrap();

        Ac97 {
            config_regs
        }
    }
}

impl PciDevice for Ac97 {
    fn config_registers(&self) -> &PciConfiguration {
        &self.config_regs
    }

    fn config_registers_mut(&mut self) -> &mut PciConfiguration {
        &mut self.config_regs
    }
}
