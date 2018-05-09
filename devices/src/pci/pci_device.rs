// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::ops::Deref;

use BusDevice;

use pci::pci_configuration::{PciConfiguration, PciHeaderType};

pub struct BarRange {
    pub addr: u64,
    pub len: u64,
}

pub trait PciDevice : BusDevice + Send + Sync {
    /// Returns the offset of `addr` in to a BAR region is a bar region contains `addr`.
    fn bar_offset(&self, addr: u64) -> Option<u64>;
    /// Gets the configuration registers of the Pci Device.
    fn config_registers(&self) -> &PciConfiguration;
    /// Gets the configuration registers of the Pci Device for modification.
    fn config_registers_mut(&mut self) -> &mut PciConfiguration;
}
