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
        let mut remaining_buffer: &mut [u8] = buffer;
        let mut cur_buffer: &mut [u8] = &mut [];
        for atrb in &self.td {
            let normal_trb = atrb.trb.cast::<NormalTrb>();
            let len = normal_trb.get_trb_transfer_length() as usize;
            let buffer_len = {
                if remaining_buffer.len() == 0 {
                    return total_size;
                }
                if remaining_buffer.len() > len {
                    len
                } else {
                    remaining_buffer.len()
                }
            };
            let (cur_buffer, remaining_buffer) = remaining_buffer.split_at_mut(buffer_len);
            total_size += self.mem.read_slice_at_addr(cur_buffer,
                                                      GuestAddress(normal_trb.get_data_buffer())).unwrap();
        }
        total_size
    }

    /// Write content from buffer, return write size.
    pub fn write(&self, buffer: &[u8]) -> usize {
        let mut total_size = 0usize;
        let mut remaining_buffer: &[u8] = buffer;
        let mut cur_buffer: &[u8] = &[];
        for atrb in &self.td {
            let normal_trb = atrb.trb.cast::<NormalTrb>();
            let len = normal_trb.get_trb_transfer_length() as usize;
            let buffer_len = {
                if remaining_buffer.len() == 0 {
                    return total_size;
                }
                if remaining_buffer.len() > len {
                    len
                } else {
                    remaining_buffer.len()
                }
            };
            let (cur_buffer, remaining_buffer) = remaining_buffer.split_at(buffer_len);
            total_size += self.mem.write_slice_at_addr(cur_buffer,
                                                      GuestAddress(normal_trb.get_data_buffer())).unwrap();
        }
        total_size
   }
}
