// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex}
use pci::pci_configuration::{
    PciClassCode, PciConfiguration, PciHeaderType
};

/// xHCI controller implementation.
pub struct Xhci {
}

impl Xhci {
    pub fn new() -> Self {
        let mut config_regs = PciConfiguration::new(
            );
    }

}
