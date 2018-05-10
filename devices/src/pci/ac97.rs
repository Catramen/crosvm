// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex};
use std::collections::BTreeMap;

use BusDevice;

use pci::pci_configuration::{PciClassCode, PciConfiguration, PciHeaderType, PciMultimediaSubclass};
use pci::pci_device::PciDevice;

// Use 82801AA because it's what qemu does.
const PCI_DEVICE_ID_INTEL_82801AA_5: u16 = 0x2415;

/// AC97 audio device emulation.
pub struct Ac97Dev {
    config_regs: PciConfiguration,
    mixer: Arc<Mutex<Ac97Mixer>>,
    bus_master: Arc<Mutex<Ac97BusMaster>>,
    audio_function: Ac97,
}

impl Ac97Dev {
    pub fn new() -> Self {
        let mut config_regs = PciConfiguration::new(0x8086,
                                                    PCI_DEVICE_ID_INTEL_82801AA_5,
                                                    PciClassCode::MultimediaController,
                                                    &PciMultimediaSubclass::AudioDevice,
                                                    PciHeaderType::Device);
        config_regs.add_io_region(0x1000, 0x0100).unwrap();
        config_regs.add_io_region(0x1400, 0x0400).unwrap();

        Ac97Dev {
            config_regs,
            mixer: Arc::new(Mutex::new(Ac97Mixer::new())),
            bus_master: Arc::new(Mutex::new(Ac97BusMaster::new())),
            audio_function: Ac97::new(),
        }
    }
}

impl PciDevice for Ac97Dev {
    fn bar_region(&self, addr: u64) -> Option<(u64, Arc<Mutex<BusDevice>>)> {
        match addr {
            a if a >= 0x1000 && a < 0x1100 => Some((addr - 0x1000, self.mixer.clone())),
            a if a >= 0x1400 && a < 0x1800 => Some((addr - 0x1000, self.bus_master.clone())),
            _ => None,
        }
    }

    fn config_registers(&self) -> &PciConfiguration {
        &self.config_regs
    }

    fn config_registers_mut(&mut self) -> &mut PciConfiguration {
        &mut self.config_regs
    }
}

struct Ac97Mixer {
    regs: BTreeMap<u8, Box<Ac97Reg>>,
}

impl Ac97Mixer {
    pub fn new() -> Self {
        Ac97Mixer {
            regs: BTreeMap::new(),
        }
    }
}

impl BusDevice for Ac97Mixer {
    fn read(&mut self, offset: u64, data: &mut [u8]) {
        println!("read from mixer");
    }

    fn write(&mut self, offset: u64, data: &[u8]) {
        println!("write to mixer");
    }
}

struct Ac97BusMaster {
    regs: BTreeMap<u8, Box<Ac97Reg>>,
}

impl Ac97BusMaster {
    pub fn new() -> Self {
        let mut regs: BTreeMap<u8, Box<Ac97Reg>> = BTreeMap::new();
        regs.insert(0x00, Box::new(PI_BDBAR::default()));
        Ac97BusMaster {
            regs,
        }
    }
}

impl BusDevice for Ac97BusMaster {
    fn read(&mut self, offset: u64, data: &mut [u8]) {
        println!("read from BM 0x{:x} {}", offset, data.len());
    }

    fn write(&mut self, offset: u64, data: &[u8]) {
        println!("write to BM 0x{:x} {}", offset, data.len());

        if let Some(reg) = self.regs.get_mut(offset) {
            if data.len() != reg.len() {
                return;
            }

            let (val, _): (u32, usize) = data.iter()
                       .fold((0, 0), |(mut v, mut shift), &b| (v | (b as u32) << shift, shift + 8));
            reg.write(val);
        }
    }
}

trait Ac97Reg : Send + Sync {
    fn read(&self, ac97: &Ac97) -> u32;
    fn write(&self, ac97: &mut Ac97, data: u32);
    /// Returns 1, 2, or 4 for byte, word, or dword respectively.
    fn len(&self) -> u8;
}

#[derive(Default)]
struct PI_BDBAR {
    val: u32,
}

impl Ac97Reg for PI_BDBAR {
    fn read(&self, ac97: &Ac97) -> u32 {
        0
    }

    fn write(&self, ac97: &mut Ac97, data: u32) {
    }

    fn len(&self) -> u8 {
        4
    }
}

// Actual audio driver controlled by the above registers.
struct Ac97 {
    // Mixer controlled settings.
    master_muted: bool,
    master_volume: u16,

    // Bus Master settings.
    buffer_descriptor_base: u32,
}

impl Ac97 {
    pub fn new() -> Self {
        Ac97 {
            master_muted: true,
            master_volume: 0,
            buffer_descriptor_base: 0,
        }
    }
}
