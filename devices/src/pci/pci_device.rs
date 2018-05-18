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
}
