// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std;
use std::boxed::Box;
use std::cmp::{min, max, Ord, Ordering, PartialEq, PartialOrd};
use std::collections::btree_map::BTreeMap;
use std::rc::Rc;
use std::ops::{Add ,Shr};
use std::mem::size_of;
use std::marker::{Copy, Sized};

type BarOffset = u64;

// This represents a range of memory in the MMIO space starting from Bar.
// Both from and to and inclusive.
#[derive(Debug, Copy, Clone)]
pub struct BarRange {
    from: BarOffset,
    to: BarOffset,
}

impl Eq for BarRange {}

impl PartialEq for BarRange {
    fn eq(&self, other: &BarRange) -> bool {
        self.from == other.from
    }
}

impl Ord for BarRange {
    fn cmp(&self, other: &BarRange) -> Ordering {
        self.from.cmp(&other.from)
    }
}

impl PartialOrd for BarRange {
    fn partial_cmp(&self, other: &BarRange) -> Option<Ordering> {
        self.from.partial_cmp(&other.from)
    }
}

impl BarRange {
    // Return if those range matches.
    pub fn overlap_with(&self, other: &BarRange) -> bool {
        if self.from > other.to || self.to < other.from {
            return false;
        }
        return true;
    }

    // Get the overlap part of two BarRange.
    // Return is (my_offset, other_offset, overlap_size).
    // For example, (4,7).overlap_range(5, 8) will be (5, 7).
    // It means, starting from 1st byte of (4,7) and 0 byte of (5, 8), there are
    // 3 bytes of overlaps.
    pub fn overlap_range(&self, other: &BarRange) -> Option<BarRange> {
        if !self.overlap_with(other) {
            return None;
        }
        Some(
            BarRange{
                from: max(self.from, other.from),
                to: min(self.to, other.to)
            }
            )
    }
}

////////////////////////////////////////////////////////////////////////////////
// Helpers for register operations.
// It's amazingly hard to specify trait bound for primitive types. So the generic
// cascading will be stopped at those helper function by using u64.
fn get_byte(val: u64, val_size: usize, offset: usize) -> u8 {
    debug_assert!(offset <= val_size);
    debug_assert!(val_size <= size_of::<u64>());
    (val >> (offset * 8)) as u8
}

fn read_reg_helper(val: u64,
                   val_size: usize,
                   val_range: BarRange,
                   addr: BarOffset,
                   data: &mut [u8]) {
        let read_range = BarRange{ from: addr, to: addr + data.len() as u64 - 1};
        if !val_range.overlap_with(&read_range) {
            // TODO(jkwang) Alarm the user.
            return;
        }
        let overlap = val_range.overlap_range(&read_range).unwrap();
        let val_start_idx = (overlap.from - val_range.from) as usize;
        let read_start_idx = (overlap.from - read_range.from) as usize;
        let total_size = (overlap.to - overlap.from) as usize + 1;
        for i in 0..total_size {
            data[read_start_idx + i] = get_byte(val, val_size, val_start_idx + i);
        }

}

// End of helpers.
////////////////////////////////////////////////////////////////////////////////
// Interface for register, as seen by guest driver.
pub trait RegisterInterface {
    fn bar_range(&self) -> BarRange;
    fn read_bar(&self, addr: BarOffset, data: &mut [u8]);
    fn write_bar(&self, addr: BarOffset, data: &[u8]) {}
    fn reset(&self) {}
    fn add_write_cb(&self, callback: Box<Fn()>) {}
}

// Spec for Hardware init Read Only Registers.
// The value of this register won't change.
pub struct StaticRegisterSpec<T> where T: std::convert::Into<u64> {
    offset: BarOffset,
    value: T,
}

// All functions implemented on this one is thread safe.
pub struct StaticRegister<T: 'static> where T: std::convert::Into<u64> {
    spec: &'static StaticRegisterSpec<T>,
}

impl<T> RegisterInterface for StaticRegister<T> where T: std::convert::Into<u64> + Clone {
    fn bar_range(&self) -> BarRange {
        BarRange {
            from: self.spec.offset,
            to: self.spec.offset + (size_of::<T>() as u64) - 1
        }
    }

    fn read_bar(&self, addr: BarOffset, data: &mut [u8]) {
        let val_range = self.bar_range();
        read_reg_helper(self.spec.value.clone().into(), size_of::<T>(), val_range, addr, data);
     }
}

#[macro_export]
macro_rules! static_register {
    (
        ty: $ty:ty,
        offset: $offset:expr,
        value: $value:expr,
    ) => {{
        static REG_SPEC: StaticRegisterSpec<$ty> = StaticRegisterSpec::<$ty> {
            offset: $offset,
            value: $value,
        };
        let r : Box<RegisterInterface> = Box::new(
            StaticRegister::<$ty> {
                spec: &REG_SPEC
            }
            );
        r
    }}
}

#[cfg(test)]
mod tests {
    use super::*;

    static REG_SPEC0: StaticRegisterSpec<u8> = StaticRegisterSpec::<u8> {
        offset: 3,
        value: 32,
    };

    static REG_SPEC1: StaticRegisterSpec<u16> = StaticRegisterSpec::<u16> {
        offset: 3,
        value: 32,
    };

    #[test]
    fn static_register_basic_test_u8() {
        let r = StaticRegister::<u8> { spec: &REG_SPEC0 };
        let mut data: [u8; 4] = [0, 0, 0, 0];
        assert_eq!(r.bar_range().from, 3);
        assert_eq!(r.bar_range().to, 3);
        r.read_bar(0, &mut data);
        assert_eq!(data, [0,0,0,32]);
        r.read_bar(2, &mut data);
        assert_eq!(data, [0,32,0,32]);
    }

    #[test]
    fn static_register_basic_test_u16() {
        let r = StaticRegister::<u16> { spec: &REG_SPEC1 };
        let mut data: [u8; 4] = [0, 0, 0, 0];
        assert_eq!(r.bar_range().from, 3);
        assert_eq!(r.bar_range().to, 4);
        r.read_bar(0, &mut data);
        assert_eq!(data, [0,0,0,32]);
        r.read_bar(2, &mut data);
        assert_eq!(data, [0,32,0,32]);
    }

    #[test]
    fn static_register_interface_test() {
        let r: Box<RegisterInterface>= static_register!{
            ty: u8,
            offset: 3,
            value: 32,
        };
        let mut data: [u8; 4] = [0, 0, 0, 0];
        assert_eq!(r.bar_range().from, 3);
        assert_eq!(r.bar_range().to, 3);
        r.read_bar(0, &mut data);
        assert_eq!(data, [0,0,0,32]);
        r.read_bar(2, &mut data);
        assert_eq!(data, [0,32,0,32]);
    }
}

/*
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
    __write_cb: RefCell<Box<Fn()>>,
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
} */
