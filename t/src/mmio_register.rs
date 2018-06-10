// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std;
use std::boxed::Box;
use std::cmp::{min, max, Ord, Ordering, PartialEq, PartialOrd};
use std::sync::{Arc, Mutex};
use std::mem::size_of;
use std::slice::{from_raw_parts_mut};

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
    fn write_bar(&self, _addr: BarOffset, _data: &[u8]) {}
    fn reset(&self) {}
}

// Spec for Hardware init Read Only Registers.
// The value of this register won't change.
pub struct StaticRegisterSpec<T> {
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
        StaticRegister::<$ty> {
            spec: &REG_SPEC
        }
    }}
}

pub struct RegisterSpec<T> {
    offset: BarOffset,
    reset_value: T,
    // Only masked bits could be written by guest.
    guest_writeable_mask: T,
    // When write 1 to bits masked, those bits will be cleared. See Xhci spec 5.1
    // for more details.
    guest_write_1_to_clear_mask: T,
}

struct RegisterInner<T: 'static> {
    value: T,
    write_cb: Option<Box<Fn(u64)>>
}

pub struct Register<T: 'static> {
    spec: &'static RegisterSpec<T>,
    inner: Arc<Mutex<RegisterInner<T>>>,
}

// All functions implemented on this one is thread safe.
impl <T> RegisterInterface for Register<T>  where T: std::convert::Into<u64> + Clone {
    fn bar_range(&self) -> BarRange {
       BarRange {
            from: self.spec.offset,
            to: self.spec.offset + (size_of::<T>() as u64) - 1
        }
    }

    fn read_bar(&self, addr: BarOffset, data: &mut [u8]) {
        let val_range = self.bar_range();
        let value = self.inner.lock().unwrap().value.clone();
        read_reg_helper(value.into(), size_of::<T>(), val_range, addr, data);
    }

    fn write_bar(&self, addr: BarOffset, data: &[u8]) {
        let my_range = self.bar_range();
        let write_range = BarRange{ from: addr, to: addr + data.len() as u64 - 1};
        if !my_range.overlap_with(&write_range) {
            // TODO(jkwang) Alarm the user.
            return;
        }
        let overlap = my_range.overlap_range(&write_range).unwrap();
        let my_start_idx = (overlap.from - my_range.from) as usize;
        let write_start_idx = (overlap.from - write_range.from) as usize;
        let total_size = (overlap.to - overlap.from) as usize + 1;
        let mut inner = self.inner.lock().unwrap();
        // Yes, it's not necessary here. But it's much easier than specify trait bounds to enable
        // shift operations.
        let value: &mut [u8] = unsafe { from_raw_parts_mut( (&mut inner.value) as *mut T as *mut u8, size_of::<T>()) };
        for i in 0..total_size {
            value[my_start_idx + i] = self.apply_write_masks_to_byte(
                value[my_start_idx + i],
                data[write_start_idx + i],
                my_start_idx + i);
        }
        if let Some(ref cb) = inner.write_cb {
            cb(inner.value.clone().into());
        }
    }

    fn reset(&self) {
        self.inner.lock().unwrap().value = self.spec.reset_value.clone();
    }
}

impl <T> Register<T> where T: std::convert::Into<u64> + Clone {
    pub fn get_value(&self) -> T{
        self.inner.lock().unwrap().value.clone()
    }

    // This function apply "write 1 to clear mask" and "guest writeable mask".
    // All write operations should go through this, the result of this function
    // is the new state of correspoding byte.
    pub fn apply_write_masks_to_byte(&self,
                                     old_byte: u8,
                                     write_byte: u8,
                                     offset: usize) -> u8 {
        let guest_write_1_to_clear_mask: u64 = self.spec.guest_write_1_to_clear_mask.clone().into();
        let guest_writeable_mask: u64 = self.spec.guest_writeable_mask.clone().into();
        // Mask with w1c mask.
        let w1c_mask = (guest_write_1_to_clear_mask >> (offset * 8)) as u8;
        let val = (!w1c_mask & write_byte) | (w1c_mask & old_byte & !write_byte);
        // Mask with writable mask.
        let w_mask = (guest_writeable_mask >> (offset * 8)) as u8;
        (old_byte & (!w_mask)) | (val & w_mask)
    }

    fn set_write_cb(&self, callback: Box<Fn(u64)>) {
        self.inner.lock().unwrap().write_cb = Some(callback);
    }

    pub fn set_value(&self, val: T) {
        self.inner.lock().unwrap().value = val;
    }
}


#[macro_export]
macro_rules! register {
    (
        ty: $ty:ty,
        offset: $offset:expr,
        reset_value: $rv:expr,
        guest_writeable_mask: $mask:expr,
        guest_write_1_to_clear_mask: $w1tcm:expr,
    ) => {{
        static REG_SPEC: RegisterSpec<$ty> = RegisterSpec::<$ty> {
            offset: $offset,
            reset_value: $rv,
            guest_writeable_mask: $mask,
            guest_write_1_to_clear_mask: $w1tcm,
        };
        Register::<$ty> {
            spec: &REG_SPEC,
            inner: Arc::new(Mutex::new(RegisterInner::<$ty> {
                value: $rv,
                write_cb: None,
            }))
        }
    }}
}

#[macro_export]
macro_rules! register_array {
    (
        ty: $ty:ty,
        cnt: $cnt:expr,
        base_offset: $base_offset:expr,
        stride: $stride:expr, // Stride of the register in bytes.
        reset_value: $rv:expr,
        guest_writeable_mask: $gwm:expr,
        guest_write_1_to_clear_mask: $gw1tcm:expr,
    ) => {{
        static mut REGS: [Register; $cnt] = [Register {
            offset: $base_offset,
            size: $size,
            reset_value: $rv,
            guest_writeable_mask: $gwm,
            guest_write_1_to_clear_mask: $gw1tcm,
        }; $cnt];
        let mut v: Vec<Register<$ty>> = Vec::new();
        for i in 0..$cnt {
            unsafe {
                REGS[i].offset += ($stride * i) as BarOffset;
                v.push(&REGS[i]);
            }
        }
        v
    }};

}

use std::collections::btree_map::BTreeMap;

pub struct MMIOSpace {
    regs: BTreeMap<BarRange, Box<RegisterInterface>>,
}

impl MMIOSpace {
    pub fn new() -> MMIOSpace {
        MMIOSpace {
            registers: BTreeMap::<BarRange, Box<RegisterInterface>::new(),
        }
    }

    /*
    pub fn add_register(&mut self, reg: RegisterInterface) {
        debug_assert_eq!(self.get_register(reg.offset).is_none(), true);
        if let Some(r) = self.first_before(reg.offset + reg.size - 1) {
            debug_assert!(r.reg.offset < reg.offset);
        }
        let reg_max_offset: usize = (reg.offset + reg.size) as usize;
        if reg_max_offset > self.data.len() {
            self.data.resize(reg_max_offset, 0);
        }

        let insert_result = self
            .registers
            .insert(reg.get_bar_range(), RegAndCallback::new(reg))
            .is_none();
        debug_assert_eq!(insert_result, true);
        reg
    }*/

    pub fn reset_all_registers(&self) {
        for (_, r) in self.registers.iter().rev() {
            r.reset()
        }
    }

    /*
    pub fn read_bar(&mut self, addr: BarOffset, data: &mut [u8]) {
        let mut offset: BarOffset = 0;
        while offset < data.len() as BarOffset {
            if let Some(ref rc) = self.get_register(addr + offset) {
                offset += rc.reg.size;
                if let Some(ref cb) = rc.cb {
                    read_cbs.push(cb.clone());
                }
            } else {
                // TODO, add logging?
                offset = offset + 1;
            }
        }
        for callback in read_cbs {
            callback.read_reg_callback(self);
        }
        for idx in 0..(data.len() as BarOffset) {
            data[idx as usize] = self.get_byte(addr + idx);
        }
    }

    pub fn write_bar(&mut self, addr: BarOffset, data: &[u8]) {
        let mut offset: BarOffset = 0;
        while offset < data.len() as BarOffset {
            if let Some(ref rc) = self.get_register(addr) {
                let mut idx: BarOffset = 0;
                while idx < (rc.reg).size && offset + idx < data.len() as BarOffset {
                    rc.reg
                        .set_byte_as_guest(self, addr + idx, data[(offset + idx) as usize]);
                    idx = idx + 1;
                }
                offset += idx;
                let val = rc.reg.get_value(self);
                if let Some(ref cb) = rc.cb {
                    cb.write_reg_callback(self, val);
                }
            } else {
                offset = offset + 1;
            }
        }
    }
    */

    fn first_before(&self, addr: BarOffset) -> Option<&Box<RegisterInterface>> {
        // for when we switch to rustc 1.17: self.devices.range(..addr).iter().rev().next()
        for (range, r) in self.registers.iter().rev() {
            if range.0 <= addr {
                return Some(r);
            }
        }
        None
    }

    /*
       fn get_register(&self, addr: BarOffset) -> Option<RegAndCallback> {
       if let Some(r) = self.first_before(addr) {
       let offset = addr - r.reg.offset;
       if offset < r.reg.size {
       return Some(r.clone());
       }
       }
       None
       }
*/
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
        let r: Box<RegisterInterface> = Box::new(static_register!{
            ty: u8,
            offset: 3,
            value: 32,
        });
        let mut data: [u8; 4] = [0, 0, 0, 0];
        assert_eq!(r.bar_range().from, 3);
        assert_eq!(r.bar_range().to, 3);
        r.read_bar(0, &mut data);
        assert_eq!(data, [0,0,0,32]);
        r.read_bar(2, &mut data);
        assert_eq!(data, [0,32,0,32]);
    }

    #[test]
    fn register_basic_rw_test() {
        let r = register! {
            ty: u8,
            offset: 3,
            reset_value: 0xf1,
            guest_writeable_mask: 0xff,
            guest_write_1_to_clear_mask: 0x0,
        };
        let mut data: [u8; 4] = [0, 0, 0, 0];
        assert_eq!(r.bar_range().from, 3);
        assert_eq!(r.bar_range().to, 3);
        r.read_bar(0, &mut data);
        assert_eq!(data, [0,0,0,0xf1]);
        r.read_bar(2, &mut data);
        assert_eq!(data, [0,0xf1,0,0xf1]);
        data = [0,0,0,0xab];
        r.write_bar(0, &data);
        assert_eq!(r.get_value(), 0xab);
        r.reset();
        assert_eq!(r.get_value(), 0xf1);
        r.set_value(0xcc);
        assert_eq!(r.get_value(), 0xcc);
    }

    #[test]
    fn register_basic_writeable_mask_test() {
        let r = register! {
            ty: u8,
            offset: 3,
            reset_value: 0x0,
            guest_writeable_mask: 0xf,
            guest_write_1_to_clear_mask: 0x0,
        };
        let mut data: [u8; 4] = [0, 0, 0, 0];
        assert_eq!(r.bar_range().from, 3);
        assert_eq!(r.bar_range().to, 3);
        r.read_bar(0, &mut data);
        assert_eq!(data, [0,0,0,0]);
        data = [0,0,0,0xab];
        r.write_bar(0, &data);
        assert_eq!(r.get_value(), 0x0b);
        r.reset();
        assert_eq!(r.get_value(), 0x0);
        r.set_value(0xcc);
        assert_eq!(r.get_value(), 0xcc);
    }

    #[test]
    fn register_basic_write_1_to_clear_mask_test() {
        let r = register! {
            ty: u8,
            offset: 3,
            reset_value: 0xf1,
            guest_writeable_mask: 0xff,
            guest_write_1_to_clear_mask: 0xf0,
        };
        let mut data: [u8; 4] = [0, 0, 0, 0];
        assert_eq!(r.bar_range().from, 3);
        assert_eq!(r.bar_range().to, 3);
        r.read_bar(0, &mut data);
        assert_eq!(data, [0,0,0,0xf1]);
        data = [0,0,0,0xfa];
        r.write_bar(0, &data);
        assert_eq!(r.get_value(), 0x0a);
        r.reset();
        assert_eq!(r.get_value(), 0xf1);
        r.set_value(0xcc);
        assert_eq!(r.get_value(), 0xcc);
    }

    #[test]
    fn register_basic_write_1_to_clear_mask_test_u32() {
        let r = register! {
            ty: u32,
            offset: 0,
            reset_value: 0xfff1,
            guest_writeable_mask: 0xff,
            guest_write_1_to_clear_mask: 0xf0,
        };
        let mut data: [u8; 4] = [0, 0, 0, 0];
        assert_eq!(r.bar_range().from, 0);
        assert_eq!(r.bar_range().to, 3);
        r.read_bar(0, &mut data);
        assert_eq!(data, [0xf1,0xff,0,0]);
        data = [0xfa,0,0,0];
        r.write_bar(0, &data);
        assert_eq!(r.get_value(), 0xff0a);
        r.reset();
        assert_eq!(r.get_value(), 0xfff1);
        r.set_value(0xcc);
        assert_eq!(r.get_value(), 0xcc);
    }


    #[test]
    fn register_callback_test() {
        let state = Arc::new(Mutex::new(0u8));
        let r = register! {
            ty: u8,
            offset: 3,
            reset_value: 0xf1,
            guest_writeable_mask: 0xff,
            guest_write_1_to_clear_mask: 0xf0,
        };

        let s2 = state.clone();
        r.set_write_cb(Box::new(
                    move |val: u64| {
                        *s2.lock().unwrap() = val as u8;
                    }
                ));
        let data: [u8; 4] = [0, 0, 0, 0xff];
        r.write_bar(0, &data);
        assert_eq!(*state.lock().unwrap(), 0xf);
        r.set_value(0xab);
        assert_eq!(*state.lock().unwrap(), 0xf);
        let data: [u8; 1] = [0xfc];
        r.write_bar(3, &data);
        assert_eq!(*state.lock().unwrap(), 0xc);
    }
}

