// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex};

use BusDevice;

use pci::pci_configuration::{PciBridgeSubclass, PciClassCode, PciConfiguration, PciHeaderType};
use pci::pci_device::PciDevice;

// Parse the CONFIG_ADDRESS register to a (enabled, bus, device, function, register) tuple.
fn parse_config_address(config_address: u32) -> (bool, usize, usize, usize, usize) {
    const BUS_NUMBER_OFFSET: usize = 16;
    const BUS_NUMBER_MASK: u32 = 0x00ff;
    const DEVICE_NUMBER_OFFSET: usize = 11;
    const DEVICE_NUMBER_MASK: u32 = 0x1f;
    const FUNCTION_NUMBER_OFFSET: usize = 8;
    const FUNCTION_NUMBER_MASK: u32 = 0x07;
    const REGISTER_NUMBER_OFFSET: usize = 2;
    const REGISTER_NUMBER_MASK: u32 = 0x3f;

    let enabled = (config_address & 0x8000_0000) != 0;
    let bus_number = ((config_address >> BUS_NUMBER_OFFSET) & BUS_NUMBER_MASK) as usize;
    let device_number = ((config_address >> DEVICE_NUMBER_OFFSET) & DEVICE_NUMBER_MASK) as usize;
    let function_number = ((config_address >> FUNCTION_NUMBER_OFFSET) & FUNCTION_NUMBER_MASK) as usize;
    let register_number = ((config_address >> REGISTER_NUMBER_OFFSET) & REGISTER_NUMBER_MASK) as usize;

    (enabled, bus_number, device_number, function_number, register_number)
}

/// Emulates the PCI Root bridge.
pub struct PciRoot {
    /// Bus configuration for the root device.
    root_configuration: PciConfiguration,
    /// Current address to read/write from (0xcf8 register, litte endian).
    config_address: u32,
    /// Devices attached to this bridge's bus.
    devices: Vec<Box<PciDevice>>,
}

impl PciRoot {
    /// Create an empty PCI root bus.
    pub fn new() -> Self {
        PciRoot {
            root_configuration: PciConfiguration::new(0, 0,
                                                      PciClassCode::BridgeDevice,
                                                      &PciBridgeSubclass::HostBridge,
                                                      PciHeaderType::Bridge),
            config_address: 0,
            devices: Vec::new(),
        }
    }

    /// Add a `PciDevice` to this root PCI bus.
    pub fn add_device(&mut self, device: Box<PciDevice>) {
        self.devices.push(device);
    }

    fn config_space_read(&self) -> u32 {
        let (enabled, bus, device, function, register) = parse_config_address(self.config_address);

        // Only support one bus.
        if !enabled || bus != 0 {
            return 0xffff_ffff;
        }

        match device {
            0 => {
                // If bus and device are both zero, then read from the root config.
                self.root_configuration.read_reg(register)
            }
            dev_num => {
                self.devices.get(dev_num - 1)
                            .map_or(0xffff_ffff,
                                    |d| d.config_registers().read_reg(register))
            }
        }
    }

    fn config_space_write(&mut self, offset: u64, data: &[u8]) {
        if offset as usize + data.len() > 4 {
            return;
        }

        let (enabled, bus, device, function, register) = parse_config_address(self.config_address);

        // Only support one bus.
        if !enabled || bus != 0 {
            return;
        }

        let regs = match device {
            0 => {
                // If bus and device are both zero, then read from the root config.
                &mut self.root_configuration
            }
            dev_num => {
                // dev_num is 1-indexed here.
                match self.devices.get_mut(dev_num - 1) {
                    Some(r) => r.config_registers_mut(),
                    None => return,
                }
            }
        };
        match data.len()  {
            1 => regs.write_byte(register * 4 + offset as usize, data[0]),
            2 => regs.write_word(register * 4 + offset as usize,
                                 (data[0] as u16) | (data[1] as u16) << 8),
            4 => regs.write_reg(register, unpack4(data)),
            _ => (),
        }
    }

    fn set_config_address(&mut self, offset: u64, data: &[u8]) {
        if offset as usize + data.len() > 4 {
            return;
        }
        let (mask, value): (u32, u32) = match data.len() {
            1 => (0x0000_00ff << (offset * 8), (data[0] as u32) << (offset * 8)),
            2 => (0x0000_ffff << (offset * 16),
                  ((data[1] as u32) << 8 | data[0] as u32) << (offset * 16)),
            4 => (0xffff_ffff, unpack4(data)),
            _ => return,
        };
        self.config_address = (self.config_address & !mask) | value;
    }
}

impl BusDevice for PciRoot {
    fn read(&mut self, offset: u64, data: &mut [u8]) {
        // `offset` is relative to 0xcf8
        let value = match offset {
            0...3 => self.config_address,
            4...7 => self.config_space_read(),
            _ => 0xffff_ffff,
        };

        // Only allow reads to the register boundary.
        let start = offset as usize % 4;
        let end = start + data.len();
        if end <= 4 {
            for i in start..end {
                data[i-start] = (value >> (i * 8)) as u8;
            }
        } else {
            for d in data {
                *d = 0xff;
            }
        }
    }

    fn write(&mut self, offset: u64, data: &[u8]) {
        // `offset` is relative to 0xcf8
        match offset {
            o @ 0...3 => self.set_config_address(o, data),
            o @ 4...7 => self.config_space_write(o - 4, data),
            _ => (),
        };
    }

    fn child_dev(&self, addr: u64) -> Option<(u64, Arc<Mutex<BusDevice>>)> {
        for d in self.devices.iter() {
            if let Some((offset, dev)) = d.bar_region(addr) {
                return Some((offset, dev.clone()));
            }
        }
        None
    }
}

fn unpack4(v: &[u8]) -> u32 {
    (v[0] as u32) | ((v[1] as u32) << 8) | ((v[2] as u32) << 16) | ((v[3] as u32) << 24)
}
