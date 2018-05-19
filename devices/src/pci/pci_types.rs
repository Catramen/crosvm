// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

pub enum PciInterruptPin {
    IntA,
    IntB,
    IntC,
    IntD,
}

impl PciInterruptPin {
    pub fn to_mask(&self) -> u32 {
        match self {
            PciInterruptPin::IntA => 0,
            PciInterruptPin::IntB => 1,
            PciInterruptPin::IntC => 2,
            PciInterruptPin::IntD => 3,
        }
    }
}
