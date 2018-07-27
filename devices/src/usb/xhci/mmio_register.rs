// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::boxed::Box;
use std::cmp::{max, min, Ord, Ordering, PartialEq, PartialOrd};
use std::convert;
use std::mem::size_of;
use std::sync::{Arc, Mutex};

use data_model::DataInit;

/// Type of offset in the bar.
pub type BarOffset = u64;

/// This represents a range of memory in the MMIO space starting from Bar.
/// Both from and to are inclusive.
#[derive(Debug, Copy, Clone)]
pub struct BarRange {
    pub from: BarOffset,
    pub to: BarOffset,
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
    /// Return true if those range overlaps.
    pub fn overlap_with(&self, other: &BarRange) -> bool {
        if self.from > other.to || self.to < other.from {
            return false;
        }
        return true;
    }

    /// Get the overlap part of two BarRange.
    /// Return is (my_offset, other_offset, overlap_size).
    /// For example, (4,7).overlap_range(5, 8) will be (5, 7).
    /// It means, starting from 1st byte of (4,7) and 0 byte of (5, 8), there are
    /// 3 bytes of overlaps.
    pub fn overlap_range(&self, other: &BarRange) -> Option<BarRange> {
        if !self.overlap_with(other) {
            return None;
        }
        Some(BarRange {
            from: max(self.from, other.from),
            to: min(self.to, other.to),
        })
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Helpers for register operations.
// It's amazingly hard to specify trait bound for primitive types. So the generic
// cascading will be stopped at those helper function by using u64.
fn get_byte(val: u64, val_size: usize, offset: usize) -> u8 {
    debug_assert!(offset <= val_size);
    debug_assert!(val_size <= size_of::<u64>());
    (val >> (offset * 8)) as u8
}

// Helper function to read a register. If the read range overlaps with value's range, it will load
// corresponding bytes into data.
fn read_reg_helper(
    val: u64,
    val_size: usize,
    val_range: BarRange,
    addr: BarOffset,
    data: &mut [u8],
) {
    let read_range = BarRange {
        from: addr,
        to: addr + data.len() as u64 - 1,
    };
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
////////////////////////////////////////////////////////////////////////////////////////////////////

/// Interface for register, as seen by guest driver.
pub trait RegisterInterface {
    /// Bar range of this register.
    fn bar_range(&self) -> BarRange;
    /// Handle read bar.
    fn read_bar(&self, addr: BarOffset, data: &mut [u8]);
    /// Handle write bar.
    fn write_bar(&self, _addr: BarOffset, _data: &[u8]) {}
    /// Reset this register to default value.
    fn reset(&self) {}
}

// Spec for hardware init Read Only Registers.
// The value of this register won't change.
pub struct StaticRegisterSpec<T: convert::Into<u64>> {
    pub offset: BarOffset,
    pub value: T,
}

/// A static register is a register inited by hardware. The value won't change in it's lifetime.
/// All functions implemented on this one is thread safe.
#[derive(Clone)]
pub struct StaticRegister<T>
where
    T: 'static + convert::Into<u64>,
{
    spec: &'static StaticRegisterSpec<T>,
}

impl<T> StaticRegister<T>
where
    T: 'static + convert::Into<u64>,
{
    /// Create an new static register from spec.
    pub fn new(spec: &'static StaticRegisterSpec<T>) -> StaticRegister<T> {
        StaticRegister { spec }
    }
}

impl<T> RegisterInterface for StaticRegister<T>
where
    T: convert::Into<u64> + Clone,
{
    fn bar_range(&self) -> BarRange {
        BarRange {
            from: self.spec.offset,
            to: self.spec.offset + (size_of::<T>() as u64) - 1,
        }
    }

    fn read_bar(&self, addr: BarOffset, data: &mut [u8]) {
        let val_range = self.bar_range();
        read_reg_helper(
            self.spec.value.clone().into(),
            size_of::<T>(),
            val_range,
            addr,
            data,
        );
    }
}

/// Macro helps to build a static register.
#[macro_export]
macro_rules! static_register {
    (ty: $ty:ty,offset: $offset:expr,value: $value:expr,) => {{
        static REG_SPEC: StaticRegisterSpec<$ty> = StaticRegisterSpec::<$ty> {
            offset: $offset,
            value: $value,
        };
        StaticRegister::<$ty>::new(&REG_SPEC)
    }};
}

/// Spec for a regular register. It specifies it's location on bar, guest writable mask and guest
/// write to clear mask.
#[derive(Clone, Copy)]
pub struct RegisterSpec<T> {
    pub offset: BarOffset,
    pub reset_value: T,
    /// Only masked bits could be written by guest.
    pub guest_writeable_mask: T,
    /// When write 1 to bits masked, those bits will be cleared. See Xhci spec 5.1
    /// for more details.
    pub guest_write_1_to_clear_mask: T,
}

struct RegisterInner<T: 'static + convert::Into<u64> + DataInit> {
    value: T,
    write_cb: Option<Box<Fn(T)>>,
}

/// Register is a thread safe struct. It can be safely changed from any thread.
#[derive(Clone)]
pub struct Register<T: 'static + convert::Into<u64> + DataInit> {
    spec: &'static RegisterSpec<T>,
    inner: Arc<Mutex<RegisterInner<T>>>,
}

impl<T> Register<T>
where
    T: convert::Into<u64> + Clone + DataInit,
{
    pub fn new(spec: &'static RegisterSpec<T>, val: T) -> Self {
        Register::<T> {
            spec,
            inner: Arc::new(Mutex::new(RegisterInner::<T> {
                value: val,
                write_cb: None,
            })),
        }
    }
}

// All functions implemented on this one is thread safe.
impl<T> RegisterInterface for Register<T>
where
    T: convert::Into<u64> + Clone + DataInit,
{
    fn bar_range(&self) -> BarRange {
        BarRange {
            from: self.spec.offset,
            to: self.spec.offset + (size_of::<T>() as u64) - 1,
        }
    }

    fn read_bar(&self, addr: BarOffset, data: &mut [u8]) {
        let val_range = self.bar_range();
        let value = self.inner.lock().unwrap().value.clone();
        read_reg_helper(value.into(), size_of::<T>(), val_range, addr, data);
    }

    fn write_bar(&self, addr: BarOffset, data: &[u8]) {
        let my_range = self.bar_range();
        let write_range = BarRange {
            from: addr,
            to: addr + data.len() as u64 - 1,
        };
        if !my_range.overlap_with(&write_range) {
            // TODO(jkwang) Alarm the user.
            return;
        }
        let overlap = my_range.overlap_range(&write_range).unwrap();
        let my_start_idx = (overlap.from - my_range.from) as usize;
        let write_start_idx = (overlap.from - write_range.from) as usize;
        let total_size = (overlap.to - overlap.from) as usize + 1;
        let mut inner = self.inner.lock().unwrap();
        // It is not necessary to use slice here. But it's much easier than adding trait bounds
        // to enable shift operations.
        {
            let value: &mut [u8] = inner.value.as_mut_slice();
            for i in 0..total_size {
                value[my_start_idx + i] = self.apply_write_masks_to_byte(
                    value[my_start_idx + i],
                    data[write_start_idx + i],
                    my_start_idx + i,
                );
            }
        }
        if let Some(ref cb) = inner.write_cb {
            cb(inner.value.clone());
        }
    }

    fn reset(&self) {
        self.inner.lock().unwrap().value = self.spec.reset_value.clone();
    }
}

impl<T> Register<T>
where
    T: convert::Into<u64> + Clone + DataInit,
{
    /// Get current value of this register.
    pub fn get_value(&self) -> T {
        self.inner.lock().unwrap().value.clone()
    }

    /// This function apply "write 1 to clear mask" and "guest writeable mask".
    /// All write operations should go through this, the result of this function
    /// is the new state of correspoding byte.
    pub fn apply_write_masks_to_byte(&self, old_byte: u8, write_byte: u8, offset: usize) -> u8 {
        let guest_write_1_to_clear_mask: u64 = self.spec.guest_write_1_to_clear_mask.clone().into();
        let guest_writeable_mask: u64 = self.spec.guest_writeable_mask.clone().into();
        // Mask with w1c mask.
        let w1c_mask = (guest_write_1_to_clear_mask >> (offset * 8)) as u8;
        let val = (!w1c_mask & write_byte) | (w1c_mask & old_byte & !write_byte);
        // Mask with writable mask.
        let w_mask = (guest_writeable_mask >> (offset * 8)) as u8;
        (old_byte & (!w_mask)) | (val & w_mask)
    }

    /// Set a callback. It will be invoked when bar write happens.
    pub fn set_write_cb<C: 'static + Fn(T)>(&self, callback: C) {
        self.inner.lock().unwrap().write_cb = Some(Box::new(callback));
    }

    /// Set value from device side. Callback won't be invoked.
    pub fn set_value(&self, val: T) {
        self.inner.lock().unwrap().value = val;
    }

    pub fn set_bits(&self, bits: T) {
        let cur_value: u64 = self.get_value().into();
        let new_value: u64 = cur_value | bits.into();
        self.set_value(new_value);
    }

    pub fn clear_bits(&self, bits: T) {
        let cur_value: u64 = self.get_value().into();
        let new_value: u64 = cur_value & (!bits.into());
        self.set_value(new_value);
    }
}

#[macro_export]
macro_rules! register {
    (
        ty:
        $ty:ty,offset:
        $offset:expr,reset_value:
        $rv:expr,guest_writeable_mask:
        $mask:expr,guest_write_1_to_clear_mask:
        $w1tcm:expr,
    ) => {{
        static REG_SPEC: RegisterSpec<$ty> = RegisterSpec::<$ty> {
            offset: $offset,
            reset_value: $rv,
            guest_writeable_mask: $mask,
            guest_write_1_to_clear_mask: $w1tcm,
        };
        Register::<$ty>::new(&REG_SPEC, $rv)
    }};
    (ty: $ty:ty,offset: $offset:expr,reset_value: $rv:expr,) => {{
        static REG_SPEC: RegisterSpec<$ty> = RegisterSpec::<$ty> {
            offset: $offset,
            reset_value: $rv,
            guest_writeable_mask: !0,
            guest_write_1_to_clear_mask: 0,
        };
        Register::<$ty>::new(&REG_SPEC, $rv)
    }};
}

#[macro_export]
macro_rules! register_array {
    (
        ty:
        $ty:ty,cnt:
        $cnt:expr,base_offset:
        $base_offset:expr,stride:
        $stride:expr,reset_value:
        $rv:expr,guest_writeable_mask:
        $gwm:expr,guest_write_1_to_clear_mask:
        $gw1tcm:expr,
    ) => {{
        static mut REGS: [RegisterSpec<$ty>; $cnt] = [RegisterSpec::<$ty> {
            offset: $base_offset,
            reset_value: $rv,
            guest_writeable_mask: $gwm,
            guest_write_1_to_clear_mask: $gw1tcm,
        }; $cnt];
        let mut v: Vec<Register<$ty>> = Vec::new();
        for i in 0..$cnt {
            unsafe {
                REGS[i].offset += ($stride * i) as BarOffset;
                v.push(Register::<$ty>::new(&REGS[i], $rv));
            }
        }
        v
    }};
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
        assert_eq!(data, [0, 0, 0, 32]);
        r.read_bar(2, &mut data);
        assert_eq!(data, [0, 32, 0, 32]);
    }

    #[test]
    fn static_register_basic_test_u16() {
        let r = StaticRegister::<u16> { spec: &REG_SPEC1 };
        let mut data: [u8; 4] = [0, 0, 0, 0];
        assert_eq!(r.bar_range().from, 3);
        assert_eq!(r.bar_range().to, 4);
        r.read_bar(0, &mut data);
        assert_eq!(data, [0, 0, 0, 32]);
        r.read_bar(2, &mut data);
        assert_eq!(data, [0, 32, 0, 32]);
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
        assert_eq!(data, [0, 0, 0, 32]);
        r.read_bar(2, &mut data);
        assert_eq!(data, [0, 32, 0, 32]);
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
        assert_eq!(data, [0, 0, 0, 0xf1]);
        r.read_bar(2, &mut data);
        assert_eq!(data, [0, 0xf1, 0, 0xf1]);
        data = [0, 0, 0, 0xab];
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
        assert_eq!(data, [0, 0, 0, 0]);
        data = [0, 0, 0, 0xab];
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
        assert_eq!(data, [0, 0, 0, 0xf1]);
        data = [0, 0, 0, 0xfa];
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
        assert_eq!(data, [0xf1, 0xff, 0, 0]);
        data = [0xfa, 0, 0, 0];
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
        r.set_write_cb(move |val: u8| {
            *s2.lock().unwrap() = val as u8;
        });
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