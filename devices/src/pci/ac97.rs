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

        let audio_function = Arc::new(Mutex::new(Ac97::new()));
        Ac97Dev {
            config_regs,
            mixer: Arc::new(Mutex::new(Ac97Mixer::new(audio_function.clone()))),
            bus_master: Arc::new(Mutex::new(Ac97BusMaster::new(audio_function))),
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
    audio_function: Arc<Mutex<Ac97>>,
}

impl Ac97Mixer {
    pub fn new(audio_function: Arc<Mutex<Ac97>>) -> Self {
        Ac97Mixer {
            audio_function,
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
    audio_function: Arc<Mutex<Ac97>>,
}

impl Ac97BusMaster {
    pub fn new(audio_function: Arc<Mutex<Ac97>>) -> Self {
        Ac97BusMaster {
            audio_function,
        }
    }
}

// Bus Master regs from ICH spec:
// 00h PI_BDBAR PCM In Buffer Descriptor list Base Address Register
// 04h PI_CIV PCM In Current Index Value
// 05h PI_LVI PCM In Last Valid Index
// 06h PI_SR PCM In Status Register
// 08h PI_PICB PCM In Position In Current Buffer
// 0Ah PI_PIV PCM In Prefetched Index Value
// 0Bh PI_CR PCM In Control Register
// 10h PO_BDBAR PCM Out Buffer Descriptor list Base Address Register
// 14h PO_CIV PCM Out Current Index Value
// 15h PO_LVI PCM Out Last Valid Index
// 16h PO_SR PCM Out Status Register
// 18h PO_PICB PCM Out Position In Current Buffer
// 1Ah PO_PIV PCM Out Prefetched Index Value
// 1Bh PO_CR PCM Out Control Register
// 20h MC_BDBAR Mic. In Buffer Descriptor list Base Address Register
// 24h PM_CIV Mic. In Current Index Value
// 25h MC_LVI Mic. In Last Valid Index
// 26h MC_SR Mic. In Status Register
// 28h MC_PICB Mic In Position In Current Buffer
// 2Ah MC_PIV Mic. In Prefetched Index Value
// 2Bh MC_CR Mic. In Control Register
// 2Ch GLOB_CNT Global Control
// 30h GLOB_STA Global Status
// 34h ACC_SEMA Codec Write Semaphore Register
impl BusDevice for Ac97BusMaster {
    fn read(&mut self, offset: u64, data: &mut [u8]) {
        println!("read from BM 0x{:x} {}", offset, data.len());
        let af = self.audio_function.lock().unwrap();
		let val: u32 = match offset {
            o @ 0x00...0x03 => read_dword(o, af.pi_bdbar),
            0x04 => af.pi_civ as u32,
            0x05 => af.pi_lvi as u32,
            o @ 0x06...0x07 => read_word(o - 0x06, af.pi_sr) as u32,
            o @ 0x08...0x09 => read_word(o - 0x08, af.pi_picb) as u32,
            0x0A => af.pi_piv as u32,
            0x0B => af.pi_cr as u32,
            o @ 0x10...0x13 => read_dword(o - 0x10, af.po_bdbar),
            0x14 => af.po_civ as u32,
            0x15 => af.po_lvi as u32,
            o @ 0x16...0x17 => read_word(o - 0x16, af.po_sr) as u32,
            o @ 0x18...0x19 => read_word(o - 0x18, af.po_picb) as u32,
            0x1a => af.po_piv as u32,
            0x1b => af.po_cr as u32,
            o @ 0x20...0x23 => read_dword(o - 0x20, af.mc_bdbar),
            0x24 => af.pm_civ as u32,
            0x25 => af.mc_lvi as u32,
            o @ 0x26...0x27 => read_word(o - 0x26, af.mc_sr) as u32,
            o @ 0x28...0x29 => read_word(o - 0x28, af.mc_picb) as u32,
            0x2a => af.mc_piv as u32,
            0x2b => af.mc_cr as u32,
            o @ 0x2c...0x2f => read_dword(o - 0x2c, af.glob_cnt),
            o @ 0x30...0x33 => read_dword(o - 0x30, af.glob_sta),
            0x34 => af.acc_sema as u32,
            _ => 0,
        };
        data.iter_mut().scan(0, |shift, b| { *b = (val >> *shift) as u8; Some(*shift + 8) });
    }

    fn write(&mut self, offset: u64, data: &[u8]) {
        println!("write to BM 0x{:x} {}", offset, data.len());
        let mut af = self.audio_function.lock().unwrap();
		match offset {
            o @ 0x00...0x03 => write_dword(o, &mut af.pi_bdbar, data),
            0x04 => (), // RO
            0x05 => af.pi_lvi = data[0],
            o @ 0x06...0x07 => write_word(o - 0x06, &mut af.pi_sr, data),
            o @ 0x08...0x09 => (), // RO
            0x0a => (), // RO
            0x0b => af.pi_cr = data[0],
            o @ 0x10...0x13 => write_dword(o - 0x10, &mut af.po_bdbar, data),
            0x14 => (), // RO
            0x15 => af.po_lvi = data[0],
            o @ 0x16...0x17 => write_word(o - 0x16, &mut af.po_sr, data),
            o @ 0x18...0x19 => (), // RO
            0x1a => af.po_piv = data[0],
            0x1b => af.po_cr = data[0],
            o @ 0x20...0x23 => write_dword(o - 0x20, &mut af.mc_bdbar, data),
            0x24 => (), // RO
            0x25 => af.mc_lvi = data[0],
            o @ 0x26...0x27 => write_word(o - 0x26, &mut af.mc_sr, data),
            o @ 0x28...0x29 => (), // RO
            0x2a => (), // RO
            0x2b => af.mc_cr = data[0],
            o @ 0x2c...0x2f => write_dword(o - 0x2c, &mut af.glob_cnt, data),
            o @ 0x30...0x33 => (), // RO
            0x34 => af.acc_sema = data[0],
            _ => (),
        }
    }
}

fn read_word(offset: u64, val: u16) -> u32 {
    match offset {
        o @ 0...1 => (val >> (8 * o) as u32) as u32,
        _ => 0,
    }
}

fn read_dword(offset: u64, val: u32) -> u32 {
    match offset {
        o @ 0...3 => (val >> (8 * o)) as u32,
        _ => 0,
    }
}

fn write_word(offset: u64, val: &mut u16, data: &[u8]) {
    match (offset as usize).checked_add(data.len()) {
        Some(n) => if n > 2 { return; }
        None => return,
    };
    match offset {
        0 => {
            if data.len() == 2 {
                *val = data[0] as u16 | ((data[1] as u16) << 8);
            }
        }
        1 => {
            if data.len() == 1 {
                *val = (*val & 0x00ff) | ((data[0] as u16) << 8);
            }
        }
        _ => (),
    }
}

fn write_dword(offset: u64, val: &mut u32, data: &[u8]) {
    match (offset as usize).checked_add(data.len()) {
        Some(n) => if n > 4 { return; }
        None => return,
    };
    for (i, d) in data.iter().enumerate() {
        let shift = offset as usize + i;
        *val = (*val & !(0x0000_00ff << shift)) | ((*d as u32) << shift);
    }
}

// Audio driver controlled by the above registers.
#[derive(Default)]
struct Ac97 {
    // Bus Master registers
    pi_bdbar: u32,
    pi_civ: u8,
    pi_lvi: u8,
    pi_sr: u16,
    pi_picb: u16,
    pi_piv: u8,
    pi_cr: u8,
    po_bdbar: u32,
    po_civ: u8,
    po_lvi: u8,
    po_sr: u16,
    po_picb: u16,
    po_piv: u8,
    po_cr: u8,
    mc_bdbar: u32,
    pm_civ: u8,
    mc_lvi: u8,
    mc_sr: u16,
    mc_picb: u16,
    mc_piv: u8,
    mc_cr: u8,
    glob_cnt: u32,
    glob_sta: u32,
    acc_sema: u8,
}

impl Ac97 {
    pub fn new() -> Self {
        Ac97 {
            pi_sr: 0x0001,
            po_sr: 0x0001,
            mc_sr: 0x0001,
            ..Default::default()
        }
    }
}
