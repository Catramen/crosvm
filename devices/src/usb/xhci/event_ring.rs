// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#[derive(Debug)]
enum Error {
    UninitializedSegTable,
    InvalidAddress
}

type Result<T> = std::result::Result<T, Error>;

// Event rings are segmented circular buffers used to pass event TRBs from the
// xHCI device back to the guest.  Each event ring is associated with a single
// interrupter.  See section 4.9.4 of the xHCI specification for more details.
pub struct EventRing {
    enqueue_pointer: GuestAddress,
    dequeue_pointer: GuestAddress,
    segment_table_address: GuestAddress,
    segment_table_size: u16,
    current_segment_index: u16,
    // Available trb count in this segment table.
    aval_trb_count: u16,
    current_entry: EventRingSegmentTable,
    guest_memory: GuestMemory,

    // Event Ring Producer Cycle state bit is initialzied to 1
    producer_cycle_state: Bit,
}

impl EventRing {
    pub fn add_event(&mut self, trb: Trb) -> Result<()> {
        if (self.segment_table_size == 0 || segment_table_address.0 == 0) {
            return Err(Error::UninitializedSegTable);
        }

        trb.set_cycle(producer_cycle_state.into());
        self.guest_memory.write_obj_at_addr(self,enqueue_pointer, trb).expect("Fail to write Guest Memory");
        self.enqueue_pointer = match self.enqueue_pointer.checked_add(TRB_SIZE) {
            Some(addr) => addr,
            None => return Err(InvalidAddress);
        }
        self.aval_trb_count -= 1;

        if self.aval_trb_count == 0 {
            self.current_segment_index += 1;
            if (
        }
    }

    pub fn is_full(&self) -> bool {
    }

    fn load_segment(&mut self) {
    }

    fn move_segment_idx(&mut self) {
    }
}

