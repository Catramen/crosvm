// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#[derive(Debug)]
enum Error {
    Uninitialized,  // The event ring is uninitialized.
    InvalidMemoryAccess, // Event ring want to do invalid memory access.
    InconstantState, // Event ring is in a bad state.
}

type Result<T> = std::result::Result<T, Error>;

// Event rings are segmented circular buffers used to pass event TRBs from the
// xHCI device back to the guest.  Each event ring is associated with a single
// interrupter.  See section 4.9.4 of the xHCI specification for more details.
pub struct EventRing {
    guest_memory: &GuestMemory,
    segment_table_size: u64,
    segment_table_base_address: GuestAddress,
    current_segment_index: u64,

    enqueue_pointer: GuestAddress,
    dequeue_pointer: GuestAddress,
    trb_count: u16,
    producer_cycle_state: bool,
}

// Public interfaces.
impl EventRing {
    pub fn new(mem: &GuestMemory) -> Self {
        EventRing {
            segment_table_size: 0,
            segment_table_base_address: GuestAddress(0),
            current_segment_index: 0,
            enqueue_pointer: GuestAddress(0),
            dequeue_pointer: GuestAddress(0),
            trb_count: 0,
            // As specified in spec 4.9.4, cycle state should be initilized to 1.
            producer_cycle_state: true,
        }
    }

    // This function implements left side of xHCI spec, Figure 4-12.
    pub fn add_event(&mut self, trb: Trb) -> Result<()> {
        self.check_inited()?;
        trb.set_cycle_bit(producer_cycle_state);
        self.guest_memory.write_obj_at_addr(self,enqueue_pointer, trb).expect("Fail to write Guest Memory");
        self.enqueue_pointer = match self.enqueue_pointer.checked_add(TRB_SIZE) {
            Some(addr) => addr,
            None => return Err(InvalidAddress);
        };
        self.trb_count -= 1;
        if self.trb_count == 0 {
            self.current_segment_index += 1;
            if self.current_segment_index == self.segment_table_size {
                self.producer_cycle_state = !self.producer_cycle_state;
                self.current_segment_index = 0;
            }
        }
        self.load_current_seg_table_entry();
    }

    pub fn set_seg_table_size(&mut self, size: u16) {
        self.segment_table_size = u16;
    }

    pub fn set_seg_table_base_addr(&mut self, addr: GuestAddress) {
        self.segment_table_base_address = addr;
        self.try_init();
    }

    pub fn set_dequeue_pointer(&mut self, addr: GuestAddress) {
        self.dequeue_pointer = addr;
    }

    pub fn isEmpty(&self) -> bool {
        return self.enqueue_pointer = self.dequeue_pointer;
    }

    // Event ring is considered full when there is only space for one last TRB.
    // In this case, xHC should write an error Trb and do a bunch of handlings.
    // See spec, figure 4-12 for more details.
    // For now, we just check event ring full and panic (as it's unlikely).
    // TODO(jkwang) Handle event ring full.
    pub fn isFull(&self) -> bool {
        if (self.trb_count == 1) {
            let next_erst_idx = (self.current_segment_index + 1) % self.segment_table_size;
            let erst_entry = self.read_seg_table_entry(next_erst_idx).unwrap();
            return self.dequeue_pointer.0 == erst.get_ring_segment_base_address;
        } else {
            return self.dequeue_pointer.0 == self.enqueue_pointer.0 + TRB_SIZE
        }
    }
}

// Private implementations.
impl EventRing {
    fn try_init(&mut self) {
        if (self.segment_table_size == 0 || segment_table_address.0 == 0) {
            return;
        }
        self.load_current_seg_table_entry().expect("Unable to init event ring");
    }

    fn check_inited(&self) -> Result<()> {
        if (self.segment_table_size == 0 ||
            self.segment_table_address == GuestAddress(0) ||
            self.enqueue_pointer == GuestAddress(0) ||
            self.dequeue_pointer == GuestAddress(0) ||
            ) {
            return Err(Error::Uninitialized);
        }
        Ok(());
    }

    fn load_current_seg_table_entry(&mut self) -> Result<()> {
        let entry = read_seg_table_entry(self.current_segment_index)?;
        self.enqueue_pointer = entry.get_ring_segment_base_address();
        self.trb_count = entry.get_ring_segment_size();
    }

    fn read_seg_table_entry(&mut self, index: u64) -> Result<EventRingSegmentTableEntry> {
        let seg_table_addr = self.get_seg_table_addr(index)?;
        let entry: EventRingSegmentTableEntry =
            self.guest_memory.read_obj_from_addr(seg_table_addr).map_err(Error::InvalidMemoryAccess)?;
        Ok(())
    }

    fn get_seg_table_addr(&self, index: u64) -> Result<GuestAddress> {
       let seg_table_addr =
            self.segment_table_base_address.checked_add(
            (SEGMENT_TABLE_SIZE as u64) * index
            );
        match seg_table_addr {
            Some(addr) => addr,
            None => return Err(Error::InvalidMemoryAccess),
        }
    }
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
    }
}
