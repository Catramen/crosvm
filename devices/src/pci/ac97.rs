// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex};
use std::collections::BTreeMap;

use BusDevice;

use pci::pci_configuration::{PciClassCode, PciConfiguration, PciHeaderType, PciMultimediaSubclass};
use pci::pci_device::PciDevice;
use sys_util::EventFd;

// Use 82801AA because it's what qemu does.
const PCI_DEVICE_ID_INTEL_82801AA_5: u16 = 0x2415;

/// AC97 audio device emulation.
pub struct Ac97Dev {
    config_regs: PciConfiguration,
    mixer: Arc<Mutex<Ac97Mixer>>,
    bus_master: Arc<Mutex<Ac97BusMaster>>,
}

impl Ac97Dev {
    pub fn new(irq_evt: EventFd, irq_num: u8) -> Self {
        let mut config_regs = PciConfiguration::new(0x8086,
                                                    PCI_DEVICE_ID_INTEL_82801AA_5,
                                                    PciClassCode::MultimediaController,
                                                    &PciMultimediaSubclass::AudioDevice,
                                                    PciHeaderType::Device);
        config_regs.add_io_region(0x1000, 0x0100).unwrap();
        config_regs.add_io_region(0x1400, 0x0400).unwrap();
        config_regs.set_irq(irq_num, 1);

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
            a if a >= 0x1400 && a < 0x1800 => Some((addr - 0x1400, self.bus_master.clone())),
            _ => None,
        }
    }

    fn config_registers(&self) -> &PciConfiguration {
        &self.config_regs
    }

    fn config_registers_mut(&mut self) -> &mut PciConfiguration {
        &mut self.config_regs
    }

//    fn get_irq_num(&self) -> u32 {
 //   }
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
//        println!("read from mixer 0x{:x} {}", offset, data.len());
    }

    fn write(&mut self, offset: u64, data: &[u8]) {
 //       println!("write to mixer 0x{:x} {}", offset, data.len());
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
//        println!("read from BM 0x{:x} {}", offset, data.len());
        let mut af = self.audio_function.lock().unwrap();
        match data.len() {
            1 => data[0] = af.bm_readb(offset),
            2 => {
                let val: u16 = af.bm_readw(offset);
                data[0] = val as u8;
                data[1] = (val >> 8) as u8;
            }
            4 => {
                let val: u32 = af.bm_readl(offset);
                data[0] = val as u8;
                data[1] = (val >> 8) as u8;
                data[2] = (val >> 16) as u8;
                data[3] = (val >> 24) as u8;
            }
            l => println!("wtf read length of {}", l)
        }
    }

    fn write(&mut self, offset: u64, data: &[u8]) {
//        println!("write to BM 0x{:x} {}", offset, data.len());
        let mut af = self.audio_function.lock().unwrap();
        match data.len() {
            1 => af.bm_writeb(offset, data[0]),
            2 => af.bm_writew(offset, data[0] as u16 | (data[1] as u16) << 8),
            4 => af.bm_writel(offset, (data[0] as u32) | ((data[1] as u32) << 8) |
                                      ((data[2] as u32) << 16) | ((data[3] as u32) << 24)),
            l => println!("wtf write length of {}", l)
        }
    }
}

// Registers for individual audio functions.
#[derive(Default)]
struct Ac97FunctionRegs {
    bdbar: u32,
    civ: u8,
    lvi: u8,
    sr: u16,
    picb: u16,
    piv: u8,
    cr: u8,
}

// Status Register Bits.
const SR_DCH: u16 = 0x01;
const SR_CELV: u16 = 0x02;
const SR_LVBCI: u16 = 0x04;
const SR_BCIS: u16 = 0x08;
const SR_FIFOE: u16 = 0x10;
const SR_VALID_MASK: u16 = 0x1f;
const SR_WCLEAR_MASK: u16 = SR_FIFOE | SR_BCIS | SR_LVBCI;
const SR_RO_MASK: u16 = SR_DCH | SR_CELV;
const SR_INT_MASK: u16 = SR_BCIS | SR_LVBCI;

// Control Register Bits.
const CR_RPBM: u8 = 0x01;
const CR_RR: u8 = 0x02;
const CR_LVBIE: u8 = 0x04;
const CR_FEIE: u8 = 0x08;
const CR_IOCE: u8 = 0x10;
const CR_VALID_MASK: u8 = 0x1f;
const CR_DONT_CLEAR_MASK: u8 = CR_IOCE | CR_FEIE | CR_LVBIE;

impl Ac97FunctionRegs {
    pub fn new() -> Self {
        Ac97FunctionRegs {
            sr: SR_DCH,
            ..Default::default()
        }
    }

    pub fn do_reset(&mut self) {
        self.bdbar = 0;
        self.civ = 0;
        self.lvi = 0;
        self.sr = SR_DCH;
        self.picb = 0;
        self.piv = 0;
        self.cr = self.cr & CR_DONT_CLEAR_MASK;
    }

    /// Read register 4, 5, and 6 as one 32 bit word.
    /// According to the ICH spec, reading these three with one 32 bit access is allowed.
    pub fn atomic_status_regs(&self) -> u32 {
        self.civ as u32 | (self.lvi as u32) << 8 | (self.sr as u32) << 16
    }
}

enum Ac97Function {
    Input,
    Output,
    Microphone,
}

// Audio driver controlled by the above registers.
struct Ac97 {
    // Bus Master registers
    pi_regs: Ac97FunctionRegs, // Input
    po_regs: Ac97FunctionRegs, // Output
    mc_regs: Ac97FunctionRegs, // Microphone
    glob_cnt: u32,
    glob_sta: u32,
    acc_sema: u8,
}

// glob_sta bits
const GS_MD3: u32 = 1 << 17;
const GS_AD3: u32 = 1 << 16;
const GS_RCS: u32 = 1 << 15;
const GS_B3S12: u32 = 1 << 14;
const GS_B2S12: u32 = 1 << 13;
const GS_B1S12: u32 = 1 << 12;
const GS_S1R1: u32 = 1 << 11;
const GS_S0R1: u32 = 1 << 10;
const GS_S1CR: u32 = 1 << 9;
const GS_S0CR: u32 = 1 << 8;
const GS_MINT: u32 = 1 << 7;
const GS_POINT: u32 = 1 << 6;
const GS_PIINT: u32 = 1 << 5;
const GS_RSRVD: u32 = 1 << 4 | 1 << 3;
const GS_MOINT: u32 = 1 << 2;
const GS_MIINT: u32 = 1 << 1;
const GS_GSCI: u32 = 1;
const GS_RO_MASK: u32 = GS_B3S12 |                   
                    GS_B2S12 |                   
                    GS_B1S12 |                   
                    GS_S1CR |                    
                    GS_S0CR |                    
                    GS_MINT |                    
                    GS_POINT |                   
                    GS_PIINT |                   
                    GS_RSRVD |                   
                    GS_MOINT |                   
                    GS_MIINT;
const GS_VALID_MASK: u32 = 0x0003_ffff;
const GS_WCLEAR_MASK: u32 = GS_RCS | GS_S1R1 | GS_S0R1 | GS_GSCI;

impl Ac97 {
    pub fn new() -> Self {
        Ac97 {
            pi_regs: Ac97FunctionRegs::new(),
            po_regs: Ac97FunctionRegs::new(),
            mc_regs: Ac97FunctionRegs::new(),
            glob_cnt: 0,
            glob_sta: 0x0000_0300, // primary and secondary codec ready set.
            acc_sema: 0,
        }
    }

    fn regs(&mut self, func: &Ac97Function) -> &Ac97FunctionRegs {
        match func {
            Ac97Function::Input => &self.pi_regs,
            Ac97Function::Output => &self.po_regs,
            Ac97Function::Microphone => &self.mc_regs,
        }
    }

    fn regs_mut(&mut self, func: &Ac97Function) -> &mut Ac97FunctionRegs {
        match func {
            Ac97Function::Input => &mut self.pi_regs,
            Ac97Function::Output => &mut self.po_regs,
            Ac97Function::Microphone => &mut self.mc_regs,
        }
    }

    fn set_bdbar(&mut self, func: Ac97Function, val: u32) {
        self.regs_mut(&func).bdbar = val & !0x03;
    }

    fn set_lvi(&mut self, func: Ac97Function, val: u8) {
        // TODO(dgreid) - handle new pointer
        self.regs_mut(&func).lvi = val % 32; // LVI wraps at 32.
    }

    fn set_sr(&mut self, func: Ac97Function, val: u16) {
        let mut sr = self.regs(&func).sr;
        if val & SR_FIFOE != 0 {
            sr &= !SR_FIFOE;
        }
        if val & SR_LVBCI != 0 {
            sr &= !SR_LVBCI;
        }
        if val & SR_BCIS != 0 {
            sr &= !SR_BCIS;
        }
        self.update_sr(&func, sr);
    }

    fn set_cr(&mut self, func: Ac97Function, val: u8) {
        let regs = self.regs_mut(&func);
        if val & CR_RR != 0 {
            regs.do_reset();

            // TODO(dgreid) stop audio
        } else {
            regs.cr = val & CR_VALID_MASK;
            if regs.cr & CR_RPBM == 0 { // Run/Pause set to pause.
                // TODO(dgreid) disable audio.
                regs.sr |= SR_DCH;
            } else { // Run/Pause set to run.
                regs.civ = regs.piv;
                regs.piv = (regs.piv + 1) % 32;
                //fetch_bd (s, r);
                regs.sr &= !SR_DCH;
                // TODO(dgreid) activate audio.
            }
        }
    }

    fn update_sr(&mut self, func: &Ac97Function, val: u16) {
        let (regs, int_mask) = match func {
            Ac97Function::Input => (&mut self.pi_regs, GS_PIINT),
            Ac97Function::Output => (&mut self.po_regs, GS_POINT),
            Ac97Function::Microphone => (&mut self.mc_regs, GS_MINT),
        };

        let mut interrupt_high = false;

        if val & SR_INT_MASK != regs.sr & SR_INT_MASK {
            if (val & SR_LVBCI) != 0 && (regs.cr & CR_LVBIE) != 0 {
                interrupt_high = true;
            }
            if (val & SR_BCIS) != 0 && (regs.cr & CR_IOCE) != 0 {
                interrupt_high = true;
            }
        }

        regs.sr = val;

        if (interrupt_high) {
            self.glob_sta |= int_mask;
            //pci_irq_assert(&s->dev);
        } else {
            self.glob_sta &= !int_mask;
            //pci_irq_deassert(&s->dev);
        }
    }

    pub fn bm_readb(&mut self, offset: u64) -> u8 {
        match offset {
            0x04 => self.pi_regs.civ,
            0x05 => self.pi_regs.lvi,
            0x0a => self.pi_regs.piv,
            0x0b => self.pi_regs.cr,
            0x14 => self.po_regs.civ,
            0x15 => self.po_regs.lvi,
            0x1a => self.po_regs.piv,
            0x1b => self.po_regs.cr,
            0x24 => self.mc_regs.civ,
            0x25 => self.mc_regs.lvi,
            0x2a => self.mc_regs.piv,
            0x2b => self.mc_regs.cr,
            0x34 => self.acc_sema,
            _ => 0,
        }
    }

    pub fn bm_readw(&mut self, offset: u64) -> u16 {
        match offset {
            0x06 => self.pi_regs.sr,
            0x08 => self.pi_regs.picb,
            0x16 => self.po_regs.sr,
            0x18 => self.po_regs.picb,
            0x26 => self.mc_regs.sr,
            0x28 => self.mc_regs.picb,
            _ => 0,
        }
    }

    pub fn bm_readl(&mut self, offset: u64) -> u32 {
        match offset {
            0x00 => self.pi_regs.bdbar,
            0x04 => self.pi_regs.atomic_status_regs(),
            0x10 => self.po_regs.bdbar,
            0x14 => self.po_regs.atomic_status_regs(),
            0x20 => self.mc_regs.bdbar,
            0x24 => self.mc_regs.atomic_status_regs(),
            0x2c => self.glob_cnt,
            0x30 => self.glob_sta,
            _ => 0,
        }
    }

    pub fn bm_writeb(&mut self, offset: u64, val: u8) {
        match offset {
            0x04 => (), // RO
            0x05 => self.set_lvi(Ac97Function::Input, val),
            0x0a => (), // RO
            0x0b => self.set_cr(Ac97Function::Input, val),
            0x14 => (), // RO
            0x15 => self.set_lvi(Ac97Function::Output, val),
            0x1a => (), // RO
            0x1b => self.set_cr(Ac97Function::Output, val),
            0x24 => (), // RO
            0x25 => self.set_lvi(Ac97Function::Microphone, val),
            0x2a => (), // RO
            0x2b => self.set_cr(Ac97Function::Microphone, val),
            0x34 => self.acc_sema = val,
            o => println!("wtf write byte to 0x{:x}", o),
        }
    }

    pub fn bm_writew(&mut self, offset: u64, val: u16) {
        match offset {
            0x06 => self.set_sr(Ac97Function::Input, val),
            0x08 => (), // RO
            0x16 => self.set_sr(Ac97Function::Output, val),
            0x18 => (), // RO
            0x26 => self.set_sr(Ac97Function::Microphone, val),
            0x28 => (), // RO
            o => println!("wtf write word to 0x{:x}", o),
        }
    }

    pub fn bm_writel(&mut self, offset: u64, val: u32) {
        match offset {
            0x00 => self.set_bdbar(Ac97Function::Input, val),
            0x10 => self.set_bdbar(Ac97Function::Output, val),
            0x20 => self.set_bdbar(Ac97Function::Microphone, val),
            0x2c => self.glob_cnt = val,
            0x30 => (), // RO
            o => println!("wtf write long to 0x{:x}", o),
        }
    }
}
