// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cmp::{Ord, Ordering, PartialEq, PartialOrd};
use std::collections::btree_map::BTreeMap;
use std::rc::Rc;

type BarOffset = u64;

// This represents a range of memory in the MMIO space starting from Bar.
// BarRange.0 is start offset, BarRange.1 is len.
#[derive(Debug, Copy, Clone)]
struct BarRange(BarOffset, BarOffset);

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

pub struct MMIOSpace {
    data: Vec<u8>,
    registers: BTreeMap<BarRange, Rc<Register>>,
}

impl MMIOSpace {
    pub fn new(size: usize) -> MMIOSpace {
        let mut v = Vec::<u8>::new();
        v.resize(size, 0);
        MMIOSpace {
            data: v,
            registers: BTreeMap::<BarRange, Rc<Register>>::new(),
        }
    }

    // This function should only be called when setup MMIOSpace.
    pub fn add_reg(&mut self, reg: Register) -> Rc<Register> {
        let reg = Rc::new(reg);
        debug_assert_eq!(self.get_register(reg.offset).is_none(), true);
        if let Some(r) = self.first_before(reg.offset + reg.size - 1) {
            debug_assert!(r.offset >= reg.offset);
        }

        let insert_result =  self.registers.insert(reg.get_bar_range(), Rc::clone(&reg)).is_none();
        debug_assert_eq!(insert_result, true);
        reg
    }

    pub fn get_register(&self, addr: BarOffset) -> Option<Rc<Register>> {
        if let Some(r) = self.first_before(addr) {
            let offset = addr - r.offset;
            if offset < r.size {
                return Some(r);
            }
        }
        None
    }

    fn first_before(&self, addr: BarOffset) -> Option<Rc<Register>> {
        // for when we switch to rustc 1.17: self.devices.range(..addr).iter().rev().next()
        for (range, reg) in self.registers.iter().rev() {
            if range.0 <= addr {
                return Some(Rc::clone(reg));
            }
        }
        None
    }

    pub fn reset_all_registers(&mut self) {
        for (_, ref mut reg) in self.registers.iter().rev() {
            reg.reset(self);
        }
    }

    pub fn read_bar(&mut self, addr: BarOffset, data: &mut [u8]) {
        let mut offset: BarOffset = 0;
        let mut read_regs = Vec::<Rc<Register>>::new();
        while offset < data.len() as BarOffset {
            if let Some(reg) = self.get_register(addr + offset) {
                offset += reg.size;
                read_regs.push(reg);
            } else {
                // TODO, add logging?
                return;
            }
        }
        for r in read_regs {
            r.callback.read_reg_callback(self);
        }
        for idx in 0..(data.len() as BarOffset) {
            data[idx as usize] = self.get_byte(addr + idx);
        }
    }

    pub fn write_bar(&mut self, addr: BarOffset, data: &[u8]) {
        let mut offset: BarOffset = 0;
        while offset < data.len() as BarOffset {
            if let Some(reg) = self.get_register(addr) {
                let mut idx: BarOffset = 0;
                while idx < (*reg).size && offset + idx < data.len() as BarOffset {
                    reg.set_byte(self, addr + idx, data[(offset + idx) as usize]);
                }
                offset += idx;
                reg.callback
                    .write_reg_callback(self, reg.get_value(self));
            } else {
                return;
            }
        }
    }

    pub fn set_byte(&mut self, addr: BarOffset, val: u8) {
        self.data[addr as usize] = val;
    }

    pub fn get_byte(&self, addr: BarOffset) -> u8 {
        self.data[addr as usize]
    }
}

// Implementing this trait will be desugared closure.
pub trait RegisterCallBack {
    fn write_reg_callback(&self, _mmioSpace: &mut MMIOSpace, _val: u64) {}

    fn read_reg_callback(&self, _mmioSpace: &mut MMIOSpace) {}
}

// Register is a piece (typically u8 to u64) of memory in MMIO Space. This struct
// denotes all information regarding to the register definition.
pub struct Register {
    offset: BarOffset,
    size: BarOffset,
    reset_value: u64,
    // Only masked bits could be written by guest.
    guest_writeable_mask: u64,
    callback: Box<RegisterCallBack>,
}

// All methods of Register should take '&self' rather than '&mut self'.
impl Register {
    pub fn get_bar_range(&self) -> BarRange {
        BarRange(self.offset, self.offset + self.size)
    }

    #[inline]
    pub fn set_byte(&self, mmioSpace: &mut MMIOSpace, offset: BarOffset, val: u8) {
        debug_assert!(offset >= self.offset);
        debug_assert!(offset - self.offset < self.size);
        debug_assert!(self.size <= 8);
        let byte_offset = offset - self.offset;
        let mask = (self.guest_writeable_mask >> (byte_offset * 8)) as u8;
        let original_val = mmioSpace.get_byte(offset);
        let new_val = (original_val & (!mask)) | (val & mask);
        mmioSpace.set_byte(offset, new_val);
    }

    pub fn reset(&self, mmioSpace: &mut MMIOSpace) {
        self.set_value_device(mmioSpace, self.reset_value);
    }

    pub fn get_value(&self, mmioSpace: &MMIOSpace) -> u64 {
        let mut val: u64 = 0;
        for byte_idx in 0..self.size {
            val = val | ((mmioSpace.get_byte(self.offset + byte_idx) as u64) << byte_idx);
        }
        val
    }

    pub fn set_value_device(&self, mmioSpace: &mut MMIOSpace, val: u64) {
        for byte_idx in 0..self.size {
            mmioSpace.set_byte(self.offset + byte_idx, (val >> byte_idx) as u8);
        }
    }

    // When a value is set from guest, it should be masked by guest_writeable_mask.
    pub fn set_value_guest(&self, mmioSpace: &mut MMIOSpace, val: u64) {
        for byte_idx in 0..self.size {
            self.set_byte(mmioSpace, self.offset + byte_idx, (val >> byte_idx) as u8);
        }
    }

    pub fn set_bit(&self, mmioSpace: &mut MMIOSpace) {
        let mut val = self.get_value(mmioSpace);
        // TODO more......
    }
}
