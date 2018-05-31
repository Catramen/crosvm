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

pub struct MMIOSpace {
    data: Vec<u8>,
    registers: BTreeMap<BarRange, Rc<Register>>,
}

impl MMIOSpace {
    pub fn new() -> MMIOSpace {
        MMIOSpace {
            data: Vec::<u8>::new(),
            registers: BTreeMap::<BarRange, Rc<Register>>::new(),
        }
    }

    pub fn get_size(&self) -> usize {
        self.data.len()
    }

    // This function should only be called when setup MMIOSpace.
    pub fn add_reg(&mut self, reg: Register) -> Rc<Register> {
        let reg = Rc::new(reg);
        debug_assert_eq!(self.get_register(reg.offset).is_none(), true);
        if let Some(r) = self.first_before(reg.offset + reg.size - 1) {
            debug_assert!(r.offset < reg.offset);
        }

        let insert_result = self
            .registers
            .insert(reg.get_bar_range(), Rc::clone(&reg))
            .is_none();
        debug_assert_eq!(insert_result, true);
        let reg_max_offset: usize = (reg.offset + reg.size) as usize;
        if reg_max_offset > self.data.len() {
            self.data.resize(reg_max_offset, 0);
        }
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

    pub fn get_all_registers(&self) -> Vec<Rc<Register>> {
        let mut v: Vec<Rc<Register>> = Vec::new();
        for (_, reg) in self.registers.iter().rev() {
            v.push(Rc::clone(reg));
        }
        v
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
        let all_regs = self.get_all_registers();
        for r in all_regs {
            r.reset(self);
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
                offset = offset + 1;
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
                    reg.set_byte_as_guest(self, addr + idx, data[(offset + idx) as usize]);
                    idx = idx + 1;
                }
                offset += idx;
                let val = reg.get_value(self);
                reg.callback.write_reg_callback(self, val);
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

// Implementing this trait will be desugared closure.
pub trait RegisterCallback {
    fn write_reg_callback(&self, _mmio_space: &mut MMIOSpace, _val: u64) {}

    fn read_reg_callback(&self, _mmio_space: &mut MMIOSpace) {}
}

pub struct DefaultCallback;
impl RegisterCallback for DefaultCallback {}

// Register is a piece (typically u8 to u64) of memory in MMIO Space. This struct
// denotes all information regarding to the register definition.
pub struct Register {
    offset: BarOffset,
    size: BarOffset,
    reset_value: u64,
    // Only masked bits could be written by guest.
    guest_writeable_mask: u64,
    callback: Box<RegisterCallback>,
}

// All methods of Register should take '&self' rather than '&mut self'.
impl Register {
    pub fn get_bar_range(&self) -> BarRange {
        BarRange(self.offset, self.offset + self.size)
    }

    #[inline]
    pub fn set_byte_as_guest(&self, mmio_space: &mut MMIOSpace, offset: BarOffset, val: u8) {
        debug_assert!(offset >= self.offset);
        debug_assert!(offset - self.offset < self.size);
        debug_assert!(self.size <= 8);
        let byte_offset = offset - self.offset;
        let mask = (self.guest_writeable_mask >> (byte_offset * 8)) as u8;
        let original_val = mmio_space.get_byte(offset);
        let new_val = (original_val & (!mask)) | (val & mask);
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

    pub fn set_bit(&self, _mmio_space: &mut MMIOSpace) {
        //let mut val = self.get_value(mmioSpace);
        // TODO more......
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn mmio_add_reg() {
        let mut mmio = MMIOSpace::new();
        mmio.add_reg(Register {
            offset: 0,
            size: 4,
            reset_value: 0,
            guest_writeable_mask: 0,
            callback: Box::new(DefaultCallback {}),
        });
        assert_eq!(mmio.get_size(), 4);
        mmio.add_reg(Register {
            offset: 32,
            size: 8,
            reset_value: 0,
            guest_writeable_mask: 0,
            callback: Box::new(DefaultCallback {}),
        });
        assert_eq!(mmio.get_size(), 40);
        mmio.add_reg(Register {
            offset: 4,
            size: 4,
            reset_value: 0,
            guest_writeable_mask: 0,
            callback: Box::new(DefaultCallback {}),
        });
        assert_eq!(mmio.get_size(), 40);
    }

    #[test]
    fn mmio_reg_read_write() {
        let mut mmio = MMIOSpace::new();
        let reg1 = mmio.add_reg(Register {
            offset: 0,
            size: 4,
            reset_value: 0,
            guest_writeable_mask: 0,
            callback: Box::new(DefaultCallback {}),
        });
        let reg2 = mmio.add_reg(Register {
            offset: 32,
            size: 1,
            reset_value: 0,
            guest_writeable_mask: 0,
            callback: Box::new(DefaultCallback {}),
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
        let reg1 = mmio.add_reg(Register {
            offset: 3,
            size: 1,
            reset_value: 0xf0,
            guest_writeable_mask: 0,
            callback: Box::new(DefaultCallback {}),
        });

        assert_eq!(reg1.get_value(&mmio), 0);
        reg1.reset(&mut mmio);
        assert_eq!(reg1.get_value(&mmio), 0xf0);
    }

    #[test]
    fn mmio_reg_guest_mask() {
        let mut mmio = MMIOSpace::new();
        let reg1 = mmio.add_reg(Register {
            offset: 3,
            size: 1,
            reset_value: 0xf0,
            guest_writeable_mask: 0,
            callback: Box::new(DefaultCallback {}),
        });
        let reg2 = mmio.add_reg(Register {
            offset: 4,
            size: 2,
            reset_value: 0x0,
            guest_writeable_mask: 0b10,
            callback: Box::new(DefaultCallback {}),
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
        reg2.set_value_guest(&mut mmio, 0xff);
        assert_eq!(reg2.get_value(&mmio), 0b11);
    }

    #[test]
    fn mmio_bar_rw() {
        let mut mmio = MMIOSpace::new();
        let reg1 = mmio.add_reg(Register {
            offset: 3,
            size: 1,
            reset_value: 0xf0,
            guest_writeable_mask: 0,
            callback: Box::new(DefaultCallback {}),
        });
        let reg2 = mmio.add_reg(Register {
            offset: 4,
            size: 2,
            reset_value: 0x0,
            guest_writeable_mask: 0b10,
            callback: Box::new(DefaultCallback {}),
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
        reg1: Rc<Register>,
    }

    struct RegCallback {
        device_state: Rc<RefCell<DeviceState>>,
    }

    impl RegisterCallback for RegCallback {
        fn write_reg_callback(&self, _: &mut MMIOSpace, val: u64) {
            self.device_state.borrow_mut().state += (val as u8);
        }

        fn read_reg_callback(&self, _: &mut MMIOSpace) {
            self.device_state.borrow_mut().state -= 1;
        }
    }

    impl Device {
        pub fn new() -> Device {
            let device_state =
                Rc::<RefCell<DeviceState>>::new(RefCell::new(DeviceState { state: 0 }));
            let mut mmio = MMIOSpace::new();
            let reg1 = mmio.add_reg(Register {
                offset: 4,
                size: 2,
                reset_value: 0x0,
                guest_writeable_mask: 0xf,
                callback: Box::new(RegCallback {
                    device_state: Rc::clone(&device_state),
                }),
            });

            let mut d = Device {
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
