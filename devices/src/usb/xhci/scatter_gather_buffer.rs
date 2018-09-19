// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::xhci_abi::{TransferDescriptor, TrbCast, TrbType, NormalTrb};
use sys_util::{GuestAddress, GuestMemory};

/// See xHCI spec 3.2.8 for scatter/gather transfer. It's used in bulk/interrupt transfers. See
/// 3.2.10 for details.
pub struct ScatterGatherBuffer {
    mem: GuestMemory,
    td: TransferDescriptor,
}

impl ScatterGatherBuffer {
    /// Create a new buffer from transfer descriptor.
    pub fn new(mem: GuestMemory, td: TransferDescriptor) -> ScatterGatherBuffer {
        for atrb in &td {
            let trb_type = atrb.trb.trb_type().unwrap();
            if trb_type != TrbType::Normal &&
                trb_type != TrbType::DataStage &&
                    trb_type != TrbType::Isoch {
                panic!("Scatter Gather buffer should only contain normal trbs, datastage trbs or isoch trbs.");
            }
        }
        ScatterGatherBuffer {
            mem,
            td,
        }
    }

    /// Total len of this buffer.
    pub fn len(&self) -> usize {
        let mut total_len = 0usize;
        for atrb in &self.td {
            total_len += atrb.trb.cast::<NormalTrb>().get_trb_transfer_length() as usize;
        }
        total_len
    }

    /// Read content to buffer, return read size.
    pub fn read(&self, buffer: &mut [u8]) -> usize {
        let mut total_size = 0usize;
        let mut offset = 0;
        for atrb in &self.td {
            let normal_trb = atrb.trb.cast::<NormalTrb>();
            let len = normal_trb.get_trb_transfer_length() as usize;
            let buffer_len = {
                if offset == buffer.len() {
                    return total_size;
                }
                if buffer.len() > offset + len {
                    len
                } else {
                    buffer.len() - offset
                }
            };
            let buffer_end = offset + buffer_len;
            let cur_buffer = &mut buffer[offset..buffer_end];
            offset = buffer_end;
            total_size += self.mem.read_slice_at_addr(cur_buffer,
                                                      GuestAddress(normal_trb.get_data_buffer())).unwrap();
        }
        total_size
    }

    /// Write content from buffer, return write size.
    pub fn write(&self, buffer: &[u8]) -> usize {
        let mut total_size = 0usize;
        let mut offset = 0;
        for atrb in &self.td {
            let normal_trb = atrb.trb.cast::<NormalTrb>();
            let len = normal_trb.get_trb_transfer_length() as usize;
            let buffer_len = {
                if offset == buffer.len() {
                    return total_size;
                }
                if buffer.len() > offset + len {
                    len
                } else {
                    buffer.len() - offset
                }
            };
            let buffer_end = offset + buffer_len;
            let cur_buffer = &buffer[offset..buffer_end];
            offset = buffer_end;
            total_size += self.mem.write_slice_at_addr(cur_buffer,
                                                      GuestAddress(normal_trb.get_data_buffer())).unwrap();
        }
        total_size
   }
}

#[cfg(test)]
mod test {
    use super::*;
    use usb::xhci::xhci_abi::{Trb, AddressedTrb};

    #[test]
    fn scatter_gather_buffer_test() {
        let gm = GuestMemory::new(&vec![(GuestAddress(0), 0x1000)]).unwrap();
        let mut td = TransferDescriptor::new();
        // In this td, we are going to have scatter buffer at 0x100, length 4, 0x200 length 2 and
        // 0x300 length 1.
        let mut trb = Trb::new();
        {
            let ntrb = trb.cast_mut::<NormalTrb>();
            ntrb.set_trb_type(TrbType::Normal as u8);
            ntrb.set_data_buffer(0x100);
            ntrb.set_trb_transfer_length(4);
        }
        td.push(AddressedTrb{trb:trb, gpa: 0});
        let mut trb = Trb::new();
        {
            let ntrb = trb.cast_mut::<NormalTrb>();
            ntrb.set_trb_type(TrbType::Normal as u8);
            ntrb.set_data_buffer(0x200);
            ntrb.set_trb_transfer_length(2);
        }
        td.push(AddressedTrb{trb:trb, gpa: 0});
        let mut trb = Trb::new();
        {
            let ntrb = trb.cast_mut::<NormalTrb>();
            ntrb.set_trb_type(TrbType::Normal as u8);
            ntrb.set_data_buffer(0x300);
            ntrb.set_trb_transfer_length(1);
        }
        td.push(AddressedTrb{trb:trb, gpa: 0});

        let buffer = ScatterGatherBuffer::new(gm.clone(), td);

        assert_eq!(buffer.len(), 7);
        let data_to_write: [u8; 7] = [7, 6, 5, 4, 3, 2, 1];
        buffer.write(&data_to_write);

        let mut d = [0; 4];
        gm.read_slice_at_addr(&mut d, GuestAddress(0x100));
        assert_eq!(d, [7,6,5,4]);;
        gm.read_slice_at_addr(&mut d, GuestAddress(0x200));
        assert_eq!(d, [3,2,0,0]);;
        gm.read_slice_at_addr(&mut d, GuestAddress(0x300));
        assert_eq!(d, [1,0,0,0]);;

        let mut data_read = [0; 7];
        buffer.read(&mut data_read);
        assert_eq!(data_to_write, data_read);
    }
}
