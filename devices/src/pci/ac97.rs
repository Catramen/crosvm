// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use pci::pci_configuration::{PciClassCode, PciConfiguration, PciHeaderType, PciMultimediaSubclass};
use pci::pci_device::PciDevice;

/// AC97 audio device emulation.
pub struct Ac97 {
    config_regs: PciConfiguration,
}

impl Ac97 {
    pub fn new() -> Self {
        let mut config_regs = PciConfiguration::new(0xf00b, 0x5050,
                                                    PciClassCode::MultimediaController,
                                                    &PciMultimediaSubclass::AudioDevice,
                                                    PciHeaderType::Device);
        config_regs.add_memory_region(0xc000_0000, 0x0001_0000).unwrap();

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
