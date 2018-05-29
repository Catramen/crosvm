// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::mem;
use std::cmp::{Ord, PartialOrd, PartialEq, Ordering};
use std::collections::btree_map::BTreeMap;

type BarOffset = u64;

// This represents a range of memory in the MMIO space starting from Bar.
// BarRange.0 is inclusive, BarRange.1 is exclusive.
#[derive(Debug, Copy, Clone)]
struct BarRange(u64, u64);

impl Eq for BarRange {}

impl PartialEq for BarRange {
    fn eq(&self, other: &BarRange) -> bool {
        self.0 == other.0
    }
}

impl Ord for BarRange {
    fn cmp(&self, other: &BarRange) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for BarRange {
    fn partial_cmp(&self, other: &BarRange) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl BarRange {
    // Subtract 'other' range from this range and return the result.
    // 'self' is the range we want to operate on.
    // 'other' is the range we operated on.
    // return value is the remaining range.
    // example:
    // self = BarRange(A, B), other = BarRange(C, D), return will be BarRange(D,B).
    //     A          B
    //     |----------|
    // C        D
    // |--------|
    //          |-----| <- we still need to deal with this range.
    // We require C <= A and D >= B. ( We are dealing with mem operations from
    // low address to high address.)
    // If all operations of range(A,B) is fullfilled by range(C,D), this function
    // will return None.
    fn subtract(&self, other: &BarRange) -> Option<BarRange> {
        debug_assert!(self.0 >= other.0);
        debug_assert!(self.0 <= other.1);
        if self.1 <= other.1 {
            return None
        }
        Some(BarRange(other.1, self.1))
    }
}

// Interface for registers/register array in MMIO space.
pub trait RegisterInterface {
    fn get_name(&self) -> &'static str;
    fn get_reset_value(&self) -> u64;
    fn get_bar_range(&self) -> BarRange;
    fn reset(&mut self);
    // Write the data to into reg. Returns the range actually written.
    fn write_reg(&mut self, addr: BarOffset, data: &[u8]) -> BarRange;
    // Read the reg values to into data. Returns the range actually read.
    fn read_reg(&self, addr: BarOffset, data: &mut [u8]) -> BarRange;
    // TODO
    fn set_write_callback(&self);
    fn set_read_callback(&self);
}

pub struct Register {
    name: &'static str,
    offset: BarOffset,
    size: u8,
    reset_value: u64,
    value: mut u64,
}

impl Register {
    fn get_byte(&self, offset: BarOffset) -> u8 {
        debug_assert!(offset < (self.size as BarOffset));
        val >> offset as u8
    }

    fn set_byte(&mut self, offset: BarOffset, val: u8) {
        debug_assert!(offset < (self.size as BarOffset));
        let mut mask: u64 = !(0xff << offset);
        self.value = self.value & mask | (val << offset);
    }

    fn get_rw_offset_and_size(&self, addr: BarOffset, len: usize) -> (u8, u8) {
        debug_assert!(addr >= self.offset);
        let start_offset_in_reg = addr - self.offset;
        let remaining_size_in_reg = self.size - start_offset_in_reg;
        let size = len < remaining_size_in_reg ? len : remaining_size_in_reg;
        (start_offset_in_reg, size)
    }
}

impl RegisterInterface for RegisterInterface {
    fn get_name(&self) -> &'static str {
        self.name
    }

    fn get_reset_value(&self) -> u64 {
        self.reset_value
    }

    fn get_bar_range(&self) -> BarRange {
        BarRange(offset, offset + size)
    }

    fn reset(&mut self) {
        self.value = self.reset_value;
    }

    fn write_reg(&mut self, addr: BarOffset, data: &[u8]) -> BarRange {
        let (offset, size) = self.get_rw_offset_and_size(addr, data.len());
        for i in 0..size {
            self.set_byte(
        }
    }

    fn read_reg(&self, addr: BarOffset, data: &mut [u8]) -> BarRange {

    }

    fn set_write_callback(&self);
    fn set_read_callback(&self);
}

pub struct RegisterArray {
    name: &'static str,

}

pub struct XhciMmioRegs {
    regs: Vec<mut Box<RegisterInterface>>,
}

impl XhciMmioRegs {
    pub fn reset_all();
    pub fn read_bar(&mut self, addr: u64, data: &mut [u8]);
    pub fn write_bar(&mut self, addr: u64, data: &[u8]);
    pub fn get_register();
}
