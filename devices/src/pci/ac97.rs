// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use BusDevice;

use pci::pci_configuration::{PciClassCode, PciConfiguration, PciHeaderType, PciMultimediaSubclass};
use pci::pci_device::{BarRange, PciDevice};

// Use 82801AA because it's what qemu does.
const PCI_DEVICE_ID_INTEL_82801AA_5: u16 = 0x2415;

/// AC97 audio device emulation.
pub struct Ac97 {
    config_regs: PciConfiguration,
    bars: Vec<BarRange>,
}

impl Ac97 {
    pub fn new() -> Self {
        let mut config_regs = PciConfiguration::new(0x8086,
                                                    PCI_DEVICE_ID_INTEL_82801AA_5,
                                                    PciClassCode::MultimediaController,
                                                    &PciMultimediaSubclass::AudioDevice,
                                                    PciHeaderType::Device);
        let bars = vec![BarRange { addr: 0x1000, len: 0x0100 },
                        BarRange { addr: 0x1400, len: 0x0400 }];

        for bar in &bars {
            config_regs.add_io_region(bar.addr, bar.len).unwrap();
        }

        Ac97 {
            config_regs,
            bars,
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

    fn bar_offset(&self, addr: u64) -> Option<u64> {
        self.bars.iter()
                 .find(|bar| bar.addr <= addr && addr < bar.addr + bar.len)
                 .map(|bar| addr - bar.addr)
    }
}

impl BusDevice for Ac97 {
    fn read(&mut self, offset: u64, data: &mut [u8]) {
    }

    fn write(&mut self, offset: u64, data: &[u8]) {
    }
}
