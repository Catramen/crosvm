// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use pci::pci_configuration::{PciConfiguration, PciHeaderType};

pub trait PciDevice : Send + Sync {
    fn config_registers(&self) -> &PciConfiguration;
    fn config_registers_mut(&mut self) -> &mut PciConfiguration;
}
