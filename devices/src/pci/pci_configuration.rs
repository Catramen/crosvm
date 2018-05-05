// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

const NUM_CONFIGURATION_REGISTERS: usize = 16;

const BAR0_REG: usize = 4;
const BAR5_REG: usize = 9;
const NUM_BAR_REGS: usize = 6;
const BAR_MEM_ADDR_MASK: u32 = 0xffff_fff0;

/// Represents the types of PCI headers allowed in the configuration registers.
pub enum PciHeaderType {
    Device,
    Bridge,
}

/// Classes of PCI nodes.
pub enum PciClassCode {
    TooOld,
    MassStorage,
    NetworkController,
    DisplayController,
    MultimediaController,
    MemoryController,
    BridgeDevice,
    SimpleCommunicationController,
    BaseSystemPeripheral,
    InputDevice,
    DockingStation,
    Processor,
    SerialBusController,
    WirelessController,
    IntelligentIoController,
    EncryptionController,
    DataAcquisitionSignalProcessing,
    Other,
}

impl PciClassCode {
    pub fn get_register_value(self) -> u8 {
        match self {
            PciClassCode::TooOld => 0x00,
            PciClassCode::MassStorage => 0x01,
            PciClassCode::NetworkController => 0x02,
            PciClassCode::DisplayController => 0x03,
            PciClassCode::MultimediaController => 0x04,
            PciClassCode::MemoryController => 0x05,
            PciClassCode::BridgeDevice => 0x06,
            PciClassCode::SimpleCommunicationController => 0x07,
            PciClassCode::BaseSystemPeripheral => 0x08,
            PciClassCode::InputDevice => 0x09,
            PciClassCode::DockingStation => 0x0a,
            PciClassCode::Processor => 0x0b,
            PciClassCode::SerialBusController => 0x0c,
            PciClassCode::WirelessController => 0x0d,
            PciClassCode::IntelligentIoController => 0x0e,
            PciClassCode::EncryptionController => 0x0f,
            PciClassCode::DataAcquisitionSignalProcessing => 0x10,
            PciClassCode::Other => 0xff,
        }
    }
}

/// A PCI sublcass. Each class in `PciClassCode` can specify a unique set of subclasses. This trait
/// is implemented by each subclass.
pub trait PciSubclass {
    /// Convert this subclass to the value used in the PCI specification.
    fn get_register_value(&self) -> u8;
}

/// Subclasses of the MultimediaController class.
pub enum PciMultimediaSubclass {
    VideoController,
    AudioController,
    TelephonyDevice,
    AudioDevice,
    Other,
}

impl PciSubclass for PciMultimediaSubclass {
    fn get_register_value(&self) -> u8 {
        match self {
            &PciMultimediaSubclass::VideoController => 0x00,
            &PciMultimediaSubclass::AudioController => 0x01,
            &PciMultimediaSubclass::TelephonyDevice => 0x02,
            &PciMultimediaSubclass::AudioDevice => 0x03,
            &PciMultimediaSubclass::Other => 0x80,
        }
    }
}

/// Subclasses of the BridgeDevice
pub enum PciBridgeSubclass {
    HostBridge,
    IsaBridge,
    EisaBridge,
    McaBridge,
    PciToPciBridge,
    PcmciaBridge,
    NuBusBridge,
    CardBusBridge,
    RACEwayBridge,
    PciToPciSemiTransparentBridge,
    InfiniBrandToPciHostBridge,
    OtherBridgeDevice,
}

impl PciSubclass for PciBridgeSubclass {
    fn get_register_value(&self) -> u8 {
        match self {
            &PciBridgeSubclass::HostBridge => 0x00,
            &PciBridgeSubclass::IsaBridge => 0x01,
            &PciBridgeSubclass::EisaBridge => 0x02,
            &PciBridgeSubclass::McaBridge => 0x03,
            &PciBridgeSubclass::PciToPciBridge => 0x04,
            &PciBridgeSubclass::PcmciaBridge => 0x05,
            &PciBridgeSubclass::NuBusBridge => 0x06,
            &PciBridgeSubclass::CardBusBridge => 0x07,
            &PciBridgeSubclass::RACEwayBridge => 0x08,
            &PciBridgeSubclass::PciToPciSemiTransparentBridge => 0x09,
            &PciBridgeSubclass::InfiniBrandToPciHostBridge => 0x0A,
            &PciBridgeSubclass::OtherBridgeDevice => 0x80,
        }
    }
}
    
/// Contains the configuration space of a PCI node.
/// See the [specification](https://en.wikipedia.org/wiki/PCI_configuration_space).
/// The configuration space is accessed by with DWORD reads and writes from the guest.
pub struct PciConfiguration {
    registers: [u32; NUM_CONFIGURATION_REGISTERS],
    writable_bits: [u32; NUM_CONFIGURATION_REGISTERS], // writable bits for each register.
    num_bars: usize,
}

impl PciConfiguration {
    pub fn new(device_id: u16, vendor_id: u16, class_code: PciClassCode, subclass: &PciSubclass,
               header_type: PciHeaderType) -> Self {
        let mut registers = [0u32; NUM_CONFIGURATION_REGISTERS];
        registers[0] = (device_id as u32) << 16 | vendor_id as u32;
        registers[2] = (class_code.get_register_value() as u32) << 24 |
                       (subclass.get_register_value() as u32) << 16;
        match header_type {
            PciHeaderType::Device => (),
            PciHeaderType::Bridge => registers[3] = 0x0001_0000,
        };
        PciConfiguration {
            registers,
            writable_bits: [0xffff_ffff; NUM_CONFIGURATION_REGISTERS],
            num_bars: 0,
        }
    }

    /// Reads a 32bit register from `reg_idx` in the register map.
    pub fn read_reg(&self, reg_idx: usize) -> u32 {
        *(self.registers.get(reg_idx)
                        .unwrap_or(&0xffff_ffff))
    }

    /// Writes a 32bit register to `reg_idx` in the register map.
    pub fn write_reg(&mut self, reg_idx: usize, value: u32) {
        let mask = self.writable_bits.get(reg_idx)
                      .map_or(0xffff_ffff, |r| *r);
        self.registers.get_mut(reg_idx)
                      .map(|r| *r = value & mask);
    }

    /// Writes a 16bit word to `offset`. `offset` must be 16bit aligned.
    pub fn write_word(&mut self, offset: usize, value: u16) {
        let shift = match offset % 4 {
            0 => 0,
            2 => 16,
            _ => return,
        };
        let mask = (0xffff as u32) << shift;
        let shifted_value = (value as u32) << shift;

        self.registers.get_mut(offset / 4)
                      .map(|r| *r = *r & !mask | shifted_value);
    }

    /// Writes a byte to `offset`.
    pub fn write_byte(&mut self, offset: usize, value: u8) {
        let shift = (offset % 4) * 8;
        let mask = (0xff as u32) << shift;
        let shifted_value = (value as u32) << shift;

        self.registers.get_mut(offset / 4)
                      .map(|r| *r = *r & !mask | shifted_value);
    }

    /// Adds a memory region of `size` at `addr`. Configures the next available BAR register to
    /// report this region and size to the guest kernel. Returns 'None' if all BARs are full, or
    /// `Some(BarIndex)` on success. `size` must be a power of 2.
    pub fn add_memory_region(&mut self, addr: u64, size: u64) -> Option<usize> {
        if self.num_bars >= NUM_BAR_REGS {
            return None;
        }
        if size.count_ones() != 1 {
            return None;
        }

        // TODO(dgreid) Allow 64 bit address and size.
        match addr.checked_add(size) {
            Some(a) => if a > u32::max_value() as u64 { return None; },
            None => return None,
        }

        let bar_idx = BAR0_REG + self.num_bars;

        self.registers[bar_idx] = addr as u32 & BAR_MEM_ADDR_MASK;
        // The first writable bit represents the size of the region.
        self.writable_bits[bar_idx] = !(size - 1) as u32;

        self.num_bars += 1;
        Some(bar_idx)
    }
}
