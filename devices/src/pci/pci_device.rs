// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex};

use BusDevice;

use pci::pci_configuration::{PciConfiguration, PciHeaderType};

pub trait PciDevice : Send + Sync {
    /// Returns the offset of `addr` in to a BAR region and the bar region that contains `addr`.
    fn bar_region(&self, addr: u64) -> Option<(u64, Arc<Mutex<BusDevice>>)>;
    /// Gets the configuration registers of the Pci Device.
    fn config_registers(&self) -> &PciConfiguration;
    /// Gets the configuration registers of the Pci Device for modification.
    fn config_registers_mut(&mut self) -> &mut PciConfiguration;
    /// Sets a register in the configuration space.
    /// * `reg_idx` - The index of the config register to modify.
    /// * `offset` - Offset in to the register.
    fn config_register_write(&mut self, reg_idx: usize, offset: u64, data: &[u8]) {
        if offset as usize + data.len() > 4 {
            return;
        }

        let regs = self.config_registers_mut();

        match data.len()  {
            1 => regs.write_byte(reg_idx * 4 + offset as usize, data[0]),
            2 => regs.write_word(reg_idx * 4 + offset as usize,
                                 (data[0] as u16) | (data[1] as u16) << 8),
            4 => regs.write_reg(reg_idx, unpack4(data)),
            _ => (),
        }
    }
    /// Gets a register from the configuration space.
    /// * `reg_idx` - The index of the config register to read.
    fn config_register_read(&self, reg_idx: usize) -> u32 {
        self.config_registers().read_reg(reg_idx)
    }
}

fn unpack4(v: &[u8]) -> u32 {
    (v[0] as u32) | ((v[1] as u32) << 8) | ((v[2] as u32) << 16) | ((v[3] as u32) << 24)
}
