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
    segment_table_size: u64,
    segment_table_base_address: GuestAddress,
    current_segment_index: u64,
    guest_memory: &GuestMemory,
    state: Option<EventRingState>,
}

// Public interfaces.
impl EventRing {
    pub fn new(mem: &GuestMemory) -> Self {
        EventRing {
            segment_table_size: 0,
            segment_table_base_address: GuestAddress(0),
            current_segment_index: 0,
            state: None,
        }
    }

    pub fn set_seg_table_size(&mut self, size: u16) {
        self.segment_table_size = u16;
    }

    pub fn set_seg_table_base_addr(&mut self, addr: GuestAddress) {
        self.segment_table_base_address = addr;
    }

}

// Private implementations.
impl EventRing {
    fn try_init(&mut self) {
        if (self.segment_table_size == 0 || segment_table_address.0 == 0) {
            return;
        }

        self.state = Some(EventRingState{

        });
    }

    fn check_inited(&self) -> Result<EventRingState> {
        match self.state {
            Some(s) => Ok(s),
            None => Err(Error::Uninitialized);
        }
    }

    fn read_current_seg_table(&mut self) -> Result<EventRingSegmentTableEntry> {
        let state = self.check_inited()?;
        let seg_table_addr = self.get_seg_table_addr()?;

        let entry: EventRingSegmentTableEntry =
            self.guest_memory.read_obj_from_addr(seg_table_addr).map_err(Error::InvalidMemoryAccess)?;
        Ok(entry)
    }

    fn get_seg_table_addr(&self) => Result<GuestAddress> {
       let seg_table_addr =
            self.segment_table_base_address.checked_add(
            (SEGMENT_TABLE_SIZE as u64) * self.current_segment_index
            );
        match seg_table_addr {
            Some(addr) => addr,
            None => return Err(Error::InvalidMemoryAccess),
        }
    }
}

struct EventRingState {
    enqueue_pointer: GuestAddress,
    dequeue_pointer: GuestAddress,
    // Available trb count in this segment table.
    aval_trb_count: u16,
    current_entry: EventRingSegmentTableEntry,
    // Event Ring Producer Cycle state bit is initialzied to 1
    producer_cycle_state: bool,
}

impl EventRing {
    pub fn new(mem: &GuestMemory) -> Self {
        EventRing {
            enqueue_pointer: GuestAddress(0),
            dequeue_pointer: GuestAddress(0),
            segment_table_address: GuestAddress(0),
            current_segment_index: 0,
            aval_trb_count: 0,
            current_entry: 0,
            // As specified in spec 4.9.4, cycle state should be initilized to 1.
            producer_cycle_state: true,
        }
    }

    pub fn add_event(&mut self, trb: Trb) -> Result<()> {
        if (self.segment_table_size == 0 || segment_table_address.0 == 0) {
            return Err(Error::UninitializedSegTable);
        }

        trb.set_cycle_bit(producer_cycle_state);
        self.guest_memory.write_obj_at_addr(self,enqueue_pointer, trb).expect("Fail to write Guest Memory");
        self.enqueue_pointer = match self.enqueue_pointer.checked_add(TRB_SIZE) {
            Some(addr) => addr,
            None => return Err(InvalidAddress);
        };
        self.aval_trb_count -= 1;

        if self.aval_trb_count == 0 {
            self.current_segment_index += 1;
            if (
        }
    }

    pub fn is_full(&self) -> bool {
    }

    fn load_segment(&mut self) {
        self.current_entry =
            self.guest_memory.read_obj_from_addr(SegmentTableEntryAddress(self.index));
        enqueue_pointer_ =
            GuestAddress(current_entry.get_ring_segment_base_address());
        self.aval_trb_count = self.current_entry.get_ring_segment_size();
    }

    fn move_segment_idx(&mut self) {
    }
}

