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
pub struct BarRange(BarOffset, BarOffset);

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

// Interface for register.
pub trait RegisterInterface {
    fn get_bar_range(&self) -> BarRange;
    fn reset(&self);
    fn read_bar(&self, addr: BarOffset, data: &mut [u8]);
    fn write_bar(&self, );
    fn set_callback(&self, );
    pub fn set_value_device(&self, mmio_space: &mut MMIOSpace, val: u64);

    // When a value is set from guest, it should be masked by guest_writeable_mask.
    pub fn set_value_guest(&self, mmio_space: &mut MMIOSpace, val: u64) {
        for byte_idx in 0..self.size {
            self.set_byte_as_guest(mmio_space, self.offset + byte_idx, (val >> byte_idx) as u8);
        }
    }

    pub fn set_bit(&self, _mmio_space: &mut MMIOSpace, _mask: u64) {
        //let mut val = self.get_value(mmioSpace);
        // TODO more......
    }

    pub fn get_bit(&self, _mmio_space: &mut MMIOSpace, _mask: u64) {
        //let mut val = self.get_value(mmioSpace);
        // TODO more......
    }
}
// Spec for Hardware init Read Only Registers.
// The value of this register won't change.
pub struct StaticRegisterSpec<T> {
    offset: BarOffset,
    size: BarOffset,
    reset_value: T,
}

pub struct RegisterSpec <T> {
    offset: BarOffset,
    size: BarOffset,
    reset_value: T,
    // Only masked bits could be written by guest.
    guest_writeable_mask: <T>,
    // When write 1 to bits masked, those bits will be cleared. See Xhci spec 5.1
    // for more details.
    guest_write_1_to_clear_mask: <T>,
}

// All functions implemented on this one is thread safe.
pub struct StaticRegister<T> {
    spec: &'static StaticRegister<T>
}

impl RegisterInterface for StaticRegister {...}

// All functions implemented on this one is thread safe.
// It can be safely cloned.
pub struct Register<T> {
    spec: &'static RegisterSpec<T>,
   // Value can be set in any thread.
    __data: Arc<Mutex<T>>
}

// Callbacks are not thread safe. They are only invoked on the thread mmio lives.
pub struct RegisterWrapper<T> {
    inner: Register<T>,
   // Write_cb can be set in the same thread.
    __write_cb: RefCell<Box<Fn(T)>>,
}

macro_rules! Register {
    () => {
        let r = Register {
            
        }
        let a : Rc<RegisterInterface> = new(...);
        (r, Rc)
    };
}
impl Register {
    pub fn set_write_callback(&mut self) {}
    pub fn write_callback(&mut self) {}
    pub fn write_value() {}
}

pub struct RegisterWrapper<T> {

}

impl RegisterInterface for RegisterWrapper {...}

pub struct MMIOSpace {
    // Owns StaticRegister, RegisterWrapper.
    regs: Rc<RegisterInterface>,
}
