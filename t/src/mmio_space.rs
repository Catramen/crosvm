// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cmp::{Ord, Ordering, PartialEq, PartialOrd};
use std::collections::btree_map::BTreeMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::marker::PhantomData;

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

#[derive(Clone)]
pub struct RegAndCallback {
    reg: &'static Register,
    cb: Option<&'static RegisterCallback>
}

impl RegAndCallback {
    pub fn new(reg:&'static Register) -> RegAndCallback {
        RegAndCallback {
            reg: reg,
            cb: None,
        }
    }
}

pub struct MMIOSpace {
    data: Vec<u8>,
    registers: BTreeMap<BarRange, RegAndCallback>,
}

impl MMIOSpace {
    pub fn new() -> MMIOSpace {
        MMIOSpace {
            data: Vec::<u8>::new(),
            registers: BTreeMap::<BarRange, RegAndCallback>::new(),
        }
    }

    pub fn get_size(&self) -> usize {
        self.data.len()
    }

    // This function should only be called when setup MMIOSpace.
    pub fn add_reg(&mut self, reg: &'static Register) {
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
    }

    pub fn add_callback(&mut self, reg: &'static Register, cb: &'static RegisterCallback) {
       for (range, r) in self.registers.iter_mut() {
            if *range == reg.get_bar_range() {
                r.cb = Some(cb);
            }
        }
    }

    pub fn get_register(&self, addr: BarOffset) -> Option<RegAndCallback> {
        if let Some(r) = self.first_before(addr) {
            let offset = addr - r.reg.offset;
            if offset < r.reg.size {
                return Some(r.clone());
            }
        }
        None
    }

    pub fn get_all_registers(&self) -> Vec<Register> {
        let mut v: Vec<Register> = Vec::new();
        for (_, rc) in self.registers.iter().rev() {
            v.push(rc.reg.clone());
        }
        v
    }

    fn first_before(&self, addr: BarOffset) -> Option<RegAndCallback> {
        // for when we switch to rustc 1.17: self.devices.range(..addr).iter().rev().next()
        for (range, r) in self.registers.iter().rev() {
            if range.0 <= addr {
                return Some(r.clone());
            }
        }
        None
    }

    pub fn reset_all_registers(&mut self) {
        let all_regs = self.get_all_registers();
        for r in all_regs {
            r.reset(self);
        }
    }

    pub fn read_bar(&mut self, addr: BarOffset, data: &mut [u8]) {
        let mut offset: BarOffset = 0;
        let mut read_cbs = Vec::<&'static RegisterCallback>::new();
        while offset < data.len() as BarOffset {
            if let Some(ref rc) = self.get_register(addr + offset) {
                offset += rc.reg.size;
                if let Some(cb) = rc.cb {
                    read_cbs.push(cb);
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
                    rc.reg.set_byte_as_guest(self, addr + idx, data[(offset + idx) as usize]);
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

    pub fn set_byte(&mut self, addr: BarOffset, val: u8) {
        // Someone is writting out of the defined mmio space.
        // TODO add log or panic?
        if (addr as usize) >= self.data.len() {
            return;
        }
        self.data[addr as usize] = val;
    }

    pub fn get_byte(&self, addr: BarOffset) -> u8 {
        // Someone is reading out of the defined mmio space.
        // TODO add log or panic?
        if (addr as usize) >= self.data.len() {
            return 0;
        }
        self.data[addr as usize]
    }
}

macro_rules! add_reg {
    ($mmio:ident, $reg_name:ident, $def:expr) => {{
    #[allow(non_upper_case_globals)]
    static $reg_name: Register = $def;
    $mmio.add_reg(&$reg_name);
    &$reg_name
}}
}

// Implementing this trait will be desugared closure.
pub trait RegisterCallback {
    fn write_reg_callback(&self, _mmio_space: &mut MMIOSpace, _val: u64) {}

    fn read_reg_callback(&self, _mmio_space: &mut MMIOSpace) {}
}

macro_rules! reg_cb {
    ($cb_name:ident, $state_name:ident) => {
        struct $cb_name {
            device_state: Rc<RefCell<$state_name>>,
        }
    };
}

pub struct RegArrayGenerator <T, C: RegArrayCallback<T>>{
    reg_count: usize,
    base_offset: BarOffset,
    stride: BarOffset, // Stride of the register in bytes.
    size: BarOffset,
    reset_value: u64,
    guest_writeable_mask: u64,
    guest_write_1_to_clear_mask: u64,
    device_state: Rc<RefCell<T>>,
    phantom: PhantomData<C>,
}

pub trait RegArrayCallback<T> {
    fn new(idx: usize, state: Rc<RefCell<T>>) -> Box<RegisterCallback>;
}

impl  <T, C: RegArrayCallback<T>> RegArrayGenerator<T, C> {
    fn generate_reg(&self, idx: usize) -> Register {
        Register {
            offset: self.base_offset + self.stride * (idx as BarOffset),
            size: self.size,
            reset_value: self.reset_value,
            guest_writeable_mask: self.guest_writeable_mask,
            guest_write_1_to_clear_mask: self.guest_write_1_to_clear_mask,
    //        callback: Some(C::new(idx, Rc::clone(&self.device_state))),
        }
    }
}

// TODO refactor Register with enum to better support Register array!
// Register is the spec of a register in mmio space. The callback 
#[derive(Clone)]
pub struct Register {
    offset: BarOffset,
    size: BarOffset,
    reset_value: u64,
    // Only masked bits could be written by guest.
    guest_writeable_mask: u64,
    // When write 1 to bits masked, those bits will be cleared. See Xhci spec 5.1
    // for more details.
    guest_write_1_to_clear_mask: u64,
}

// All methods of Register should take '&self' rather than '&mut self'.
impl Register {
    pub fn new_ro(offset: BarOffset, size: BarOffset, reset_value: u64) -> Register {
        Register {
            offset: offset,
            size: size,
            reset_value: reset_value,
            guest_writeable_mask: 0,
            guest_write_1_to_clear_mask: 0,
        }
    }

    pub fn get_bar_range(&self) -> BarRange {
        BarRange(self.offset, self.offset + self.size)
    }

    #[inline]
    pub fn set_byte_as_guest(&self, mmio_space: &mut MMIOSpace, offset: BarOffset, val: u8) {
        debug_assert!(offset >= self.offset);
        debug_assert!(offset - self.offset < self.size);
        debug_assert!(self.size <= 8);
        let byte_offset = offset - self.offset;
        let original_val = mmio_space.get_byte(offset);
        // Mask with w1c mask.
        let w1c_mask = (self.guest_write_1_to_clear_mask >> (byte_offset * 8)) as u8;
        let val = (!w1c_mask & val) | (w1c_mask & original_val & !val);
        // Mask with writable mask.
        let w_mask = (self.guest_writeable_mask >> (byte_offset * 8)) as u8;
        let new_val = (original_val & (!w_mask)) | (val & w_mask);
        mmio_space.set_byte(offset, new_val);
    }

    pub fn reset(&self, mmio_space: &mut MMIOSpace) {
        self.set_value_device(mmio_space, self.reset_value);
    }

    pub fn get_value(&self, mmio_space: &MMIOSpace) -> u64 {
        let mut val: u64 = 0;
        for byte_idx in 0..self.size {
            val = val | ((mmio_space.get_byte(self.offset + byte_idx) as u64) << (byte_idx * 8));
        }
        val
    }

    pub fn set_value_device(&self, mmio_space: &mut MMIOSpace, val: u64) {
        for byte_idx in 0..self.size {
            mmio_space.set_byte(self.offset + byte_idx, (val >> (byte_idx * 8)) as u8);
        }
    }

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

    pub fn set_to_clear(&self, _mmio_space: &mut MMIOSpace, _mask: u64) {
        //let mut val = self.get_value(mmioSpace);
        // TODO more......
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mmio_add_reg() {
        let mut mmio = MMIOSpace::new();
        add_reg!(mmio, reg1, Register {
            offset: 0,
            size: 4,
            reset_value: 0,
            guest_writeable_mask: 0,
            guest_write_1_to_clear_mask: 0,
        });
        assert_eq!(mmio.get_size(), 4);
        add_reg!(mmio, reg2, Register {
            offset: 32,
            size: 8,
            reset_value: 0,
            guest_writeable_mask: 0,
            guest_write_1_to_clear_mask: 0,
        });
        assert_eq!(mmio.get_size(), 40);
        add_reg!(mmio, reg3, Register {
            offset: 4,
            size: 4,
            reset_value: 0,
            guest_writeable_mask: 0,
            guest_write_1_to_clear_mask: 0,
        });
        assert_eq!(mmio.get_size(), 40);
    }

    #[test]
    fn mmio_reg_read_write() {
        let mut mmio = MMIOSpace::new();
        let reg1 = add_reg!(mmio, reg1, Register {
            offset: 0,
            size: 4,
            reset_value: 0,
            guest_writeable_mask: 0,
            guest_write_1_to_clear_mask: 0,
        });
        let reg2 = add_reg!(mmio, reg2, Register {
            offset: 32,
            size: 1,
            reset_value: 0,
            guest_writeable_mask: 0,
            guest_write_1_to_clear_mask: 0,
        });
        assert_eq!(reg1.get_value(&mmio), 0);
        assert_eq!(reg2.get_value(&mmio), 0);
        // Only last 4 bytes will be set as reg1 size is 4.
        reg1.set_value_device(&mut mmio, 0xf0f0f0f0f0f0f0f0);
        assert_eq!(mmio.data[0], 0xf0);
        assert_eq!(mmio.data[1], 0xf0);
        assert_eq!(mmio.data[2], 0xf0);
        assert_eq!(mmio.data[3], 0xf0);
        assert_eq!(mmio.data[4], 0x0);
        assert_eq!(reg1.get_value(&mmio), 0xf0f0f0f0);

        reg2.set_value_device(&mut mmio, 0xf0f0);
        assert_eq!(mmio.data[32], 0xf0);
        assert_eq!(reg2.get_value(&mmio), 0xf0);
    }

    #[test]
    fn mmio_reg_reset() {
        let mut mmio = MMIOSpace::new();
        let reg1 = add_reg!(mmio, reg1, Register {
            offset: 3,
            size: 1,
            reset_value: 0xf0,
            guest_writeable_mask: 0,
            guest_write_1_to_clear_mask: 0,
        });

        assert_eq!(reg1.get_value(&mmio), 0);
        reg1.reset(&mut mmio);
        assert_eq!(reg1.get_value(&mmio), 0xf0);
    }

    #[test]
    fn mmio_reg_guest_mask() {
        let mut mmio = MMIOSpace::new();
        let reg1 = add_reg!(mmio, reg1, Register {
            offset: 3,
            size: 1,
            reset_value: 0xf0,
            guest_writeable_mask: 0,
            guest_write_1_to_clear_mask: 0,
        });
        let reg2 = add_reg!(mmio, reg2, Register {
            offset: 4,
            size: 2,
            reset_value: 0x0,
            guest_writeable_mask: 0b10,
            guest_write_1_to_clear_mask: 0,
        });
        assert_eq!(reg1.get_value(&mmio), 0);
        reg1.set_value_guest(&mut mmio, 0xffffff);
        assert_eq!(reg1.get_value(&mmio), 0x0);
        assert_eq!(reg2.get_value(&mmio), 0);
        reg2.set_value_device(&mut mmio, 0xff);
        assert_eq!(reg2.get_value(&mmio), 0xff);
        reg2.set_value_guest(&mut mmio, 0x0);
        assert_eq!(reg2.get_value(&mmio), 0xff & (!0b10));
        reg2.set_value_device(&mut mmio, 0b1);
        reg2.set_value_guest(&mut mmio, 0xffff);
        assert_eq!(reg2.get_value(&mmio), 0b11);
    }

    #[test]
    fn mmio_reg_write_1_to_clear_mask() {
        let mut mmio = MMIOSpace::new();
        let reg1 = add_reg!(mmio, reg1, Register {
            offset: 3,
            size: 1,
            reset_value: 0xf0,
            guest_writeable_mask: 0b1,
            guest_write_1_to_clear_mask: 0b1,
        });
        let reg2 = add_reg!(mmio, reg2, Register {
            offset: 4,
            size: 2,
            reset_value: 0x0,
            guest_writeable_mask: 0b11,
            guest_write_1_to_clear_mask: 0b01,
        });
        reg1.set_value_device(&mut mmio, 0xff);
        reg1.set_value_guest(&mut mmio, 0xffff);
        assert_eq!(reg1.get_value(&mmio), 0xfe);
        reg2.set_value_device(&mut mmio, 0b1);
        reg2.set_value_guest(&mut mmio, 0xffff);
        assert_eq!(reg2.get_value(&mmio), 0b10);
    }

    #[test]
    fn mmio_bar_rw() {
        let mut mmio = MMIOSpace::new();
        let reg1 = add_reg!(mmio, reg1, Register {
            offset: 3,
            size: 1,
            reset_value: 0xf0,
            guest_writeable_mask: 0,
            guest_write_1_to_clear_mask: 0,
        });
        let reg2 = add_reg!(mmio, reg2, Register {
            offset: 4,
            size: 2,
            reset_value: 0x0,
            guest_writeable_mask: 0b10,
            guest_write_1_to_clear_mask: 0,
        });
        let mut buffer: [u8; 4] = [0; 4];
        mmio.read_bar(0, &mut buffer);
        assert_eq!(buffer, [0, 0, 0, 0]);
        mmio.read_bar(4, &mut buffer);
        assert_eq!(buffer, [0, 0, 0, 0]);

        reg1.reset(&mut mmio);
        assert_eq!(reg1.get_value(&mmio), 0xf0);
        mmio.read_bar(0, &mut buffer);
        assert_eq!(buffer, [0, 0, 0, 0xf0]);
        // This write will have no effect cause of guset_writeable_mask.
        mmio.write_bar(0, &[0xff, 0xff, 0xff, 0xff]);
        assert_eq!(reg1.get_value(&mmio), 0xf0);
        mmio.read_bar(0, &mut buffer);
        assert_eq!(buffer, [0, 0, 0, 0xf0]);

        mmio.write_bar(4, &[0xff, 0xff, 0xff, 0xff]);
        mmio.read_bar(4, &mut buffer);
        assert_eq!(buffer, [0b10, 0, 0, 0]);
        reg2.set_value_device(&mut mmio, 0xf);
        mmio.write_bar(4, &[0, 0, 0, 0]);
        mmio.read_bar(4, &mut buffer);
        assert_eq!(buffer, [0b1101, 0, 0, 0]);
    }

    // The following test demonstrate how to use register call back to cause side
    // effect.
    struct DeviceState {
        state: u8,
    }

    struct Device {
        mmio_space: MMIOSpace,
        state: Rc<RefCell<DeviceState>>,
        reg1: &'static Register,
    }

    reg_cb!(RegCallback, DeviceState);
    impl RegisterCallback for RegCallback {
        fn write_reg_callback(&self, _mmio: &mut MMIOSpace, val: u64) {
            self.device_state.borrow_mut().state += val as u8;
        }

        fn read_reg_callback(&self, _mmio: &mut MMIOSpace)
        {
            self.device_state.borrow_mut().state -= 1;
        }
    }

    impl Device {
        pub fn new() -> Device {
            let device_state =
                Rc::<RefCell<DeviceState>>::new(RefCell::new(DeviceState { state: 0 }));
            let mut mmio = MMIOSpace::new();
            let reg1 = add_reg!(mmio, reg1, Register {
                offset: 4,
                size: 2,
                reset_value: 0x0,
                guest_writeable_mask: 0xf,
                guest_write_1_to_clear_mask: 0,
            });

            let cb = unsafe {
                static mut callback: Option<RegCallback> = None;
                callback = Some(RegCallback {
                    device_state: Rc::clone(&device_state),
                });
                match callback {
                    Some(ref cb) => cb,
                    None => panic!("This should never happen!"),
                }
            };
            mmio.add_callback(reg1, cb);
            let d = Device {
                mmio_space: mmio,
                state: device_state,
                reg1: reg1,
            };
            d
        }
    }

    #[test]
    fn mmio_reg_callback() {
        let mut d = Device::new();
        assert_eq!(d.state.borrow().state, 0);
        // No side effect when device write the register.
        d.reg1.set_value_device(&mut d.mmio_space, 0xff);
        assert_eq!(d.state.borrow().state, 0);
        // No side effect when write goes through the mask.
        d.reg1.set_value_guest(&mut d.mmio_space, 0x0);
        assert_eq!(d.state.borrow().state, 0x0);
        // Side effect only happens when write_bar.
        d.mmio_space.write_bar(4, &[0, 0]);
        assert_eq!(d.state.borrow().state, 0xf0);
        let mut read_buffer: [u8; 2] = [0;2];
        d.mmio_space.read_bar(4, &mut read_buffer);
        assert_eq!(d.state.borrow().state, 0xf0 - 1);
    }


}
