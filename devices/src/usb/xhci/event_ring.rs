// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std;
use std::mem::size_of;
use sys_util::{GuestAddress, GuestMemory};

use super::xhci_abi::*;

#[derive(Debug, PartialEq)]
pub enum Error {
    Uninitialized,       // The event ring is uninitialized.
    InvalidMemoryAccess, // Event ring want to do invalid memory access.
    InconstantState,     // Event ring is in a bad state.
    EventRingFull,       // Event ring is full.
}

type Result<T> = std::result::Result<T, Error>;

/// Event rings are segmented circular buffers used to pass event TRBs from the xHCI device back to
/// the guest.  Each event ring is associated with a single interrupter.  See section 4.9.4 of the
/// xHCI specification for more details.
pub struct EventRing {
    mem: GuestMemory,
    segment_table_size: u16,
    segment_table_base_address: GuestAddress,
    current_segment_index: u16,
    trb_count: u16,
    enqueue_pointer: GuestAddress,
    dequeue_pointer: GuestAddress,
    producer_cycle_state: bool,
}

impl EventRing {
    /// Create an empty, uninited event ring.
    pub fn new(mem: GuestMemory) -> Self {
        EventRing {
            mem: mem,
            segment_table_size: 0,
            segment_table_base_address: GuestAddress(0),
            current_segment_index: 0,
            enqueue_pointer: GuestAddress(0),
            dequeue_pointer: GuestAddress(0),
            trb_count: 0,
            // As specified in xHCI spec 4.9.4, cycle state should be initilized to 1.
            producer_cycle_state: true,
        }
    }

    /// This function implements left side of xHCI spec, Figure 4-12.
    pub fn add_event(&mut self, mut trb: Trb) -> Result<()> {
        self.check_inited()?;
        if self.is_full().unwrap() == true {
            return Err(Error::EventRingFull);
        }
        trb.set_cycle_bit(self.producer_cycle_state);
        self.mem
            .write_obj_at_addr(trb, self.enqueue_pointer)
            .expect("Fail to write Guest Memory");
        self.enqueue_pointer = match self.enqueue_pointer.checked_add(size_of::<Trb>() as u64) {
            Some(addr) => addr,
            None => return Err(Error::InconstantState),
        };
        self.trb_count -= 1;
        if self.trb_count == 0 {
            self.current_segment_index += 1;
            if self.current_segment_index == self.segment_table_size {
                self.producer_cycle_state = !self.producer_cycle_state;
                self.current_segment_index = 0;
            }
            self.load_current_seg_table_entry()?;
        }
        Ok(())
    }

    /// Set segment table size.
    pub fn set_seg_table_size(&mut self, size: u16) {
        self.segment_table_size = size;
    }

    /// Set segment table base addr.
    pub fn set_seg_table_base_addr(&mut self, addr: GuestAddress) {
        self.segment_table_base_address = addr;
        self.try_init();
    }

    /// Set dequeue pointer.
    pub fn set_dequeue_pointer(&mut self, addr: GuestAddress) {
        self.dequeue_pointer = addr;
    }

    /// Get the enqueue pointer.
    pub fn get_enqueue_pointer(&self) -> GuestAddress {
        self.enqueue_pointer
    }

    /// Check if event ring is empty.
    pub fn is_empty(&self) -> Result<bool> {
        self.check_inited()?;
        Ok(self.enqueue_pointer == self.dequeue_pointer)
    }

    /// Event ring is considered full when there is only space for one last TRB. In this case, xHC
    /// should write an error Trb and do a bunch of handlings. See spec, figure 4-12 for more
    /// details.
    /// For now, we just check event ring full and panic (as it's unlikely to happen).
    /// TODO(jkwang) Handle event ring full.
    pub fn is_full(&self) -> Result<bool> {
        self.check_inited()?;
        if self.trb_count == 1 {
            // erst == event ring segment table
            let next_erst_idx = (self.current_segment_index + 1) % self.segment_table_size;
            let erst_entry = self.read_seg_table_entry(next_erst_idx).unwrap();
            Ok(self.dequeue_pointer.0 == erst_entry.get_ring_segment_base_address())
        } else {
            Ok(self.dequeue_pointer.0 == self.enqueue_pointer.0 + size_of::<Trb>() as u64)
        }
    }

    /// Try to init event ring. Will fail if seg table size/address are invalid.
    fn try_init(&mut self) {
        if self.segment_table_size == 0 || self.segment_table_base_address.0 == 0 {
            return;
        }
        self.load_current_seg_table_entry()
            .expect("Unable to init event ring");
    }

    // Check if this event ring is inited.
    fn check_inited(&self) -> Result<()> {
        if self.segment_table_size == 0
            || self.segment_table_base_address == GuestAddress(0)
            || self.enqueue_pointer == GuestAddress(0)
        {
            return Err(Error::Uninitialized);
        }
        Ok(())
    }

    // Load entry of current seg table.
    fn load_current_seg_table_entry(&mut self) -> Result<()> {
        let entry = self.read_seg_table_entry(self.current_segment_index)?;
        self.enqueue_pointer = GuestAddress(entry.get_ring_segment_base_address());
        self.trb_count = entry.get_ring_segment_size();
        Ok(())
    }

    // Get seg table entry at index.
    fn read_seg_table_entry(&self, index: u16) -> Result<EventRingSegmentTableEntry> {
        let seg_table_addr = self.get_seg_table_addr(index)?;
        let entry: EventRingSegmentTableEntry = self
            .mem
            .read_obj_from_addr(seg_table_addr)
            .map_err(|_e| Error::InvalidMemoryAccess)?;
        Ok(entry)
    }

    // Get seg table addr at index.
    fn get_seg_table_addr(&self, index: u16) -> Result<GuestAddress> {
        let seg_table_addr = self
            .segment_table_base_address
            .checked_add(((size_of::<EventRingSegmentTableEntry>() as u16) * index) as u64);
        match seg_table_addr {
            Some(addr) => Ok(addr),
            None => return Err(Error::InvalidMemoryAccess),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn test_uninited() {
        let gm = GuestMemory::new(&vec![(GuestAddress(0), 0x1000)]).unwrap();
        let mut er = EventRing::new(gm.clone());
        let trb = Trb::new();
        assert_eq!(er.add_event(trb), Err(Error::Uninitialized));
        assert_eq!(er.is_empty(), Err(Error::Uninitialized));
        assert_eq!(er.is_full(), Err(Error::Uninitialized));
    }

    #[test]
    fn test_event_ring() {
        let trb_size = size_of::<Trb>() as u64;
        let gm = GuestMemory::new(&vec![(GuestAddress(0), 0x1000)]).unwrap();
        let mut er = EventRing::new(gm.clone());
        let mut st_entries = [EventRingSegmentTableEntry::new(); 3];
        st_entries[0].set_ring_segment_base_address(0x100);
        st_entries[0].set_ring_segment_size(3);
        st_entries[1].set_ring_segment_base_address(0x200);
        st_entries[1].set_ring_segment_size(3);
        st_entries[2].set_ring_segment_base_address(0x300);
        st_entries[2].set_ring_segment_size(3);
        gm.write_obj_at_addr(st_entries[0], GuestAddress(0x8))
            .unwrap();
        gm.write_obj_at_addr(
            st_entries[1],
            GuestAddress(0x8 + size_of::<EventRingSegmentTableEntry>() as u64),
        ).unwrap();
        gm.write_obj_at_addr(
            st_entries[2],
            GuestAddress(0x8 + 2 * size_of::<EventRingSegmentTableEntry>() as u64),
        ).unwrap();
        // Init event ring. Must init after segment tables writting.
        er.set_seg_table_size(3);
        er.set_seg_table_base_addr(GuestAddress(0x8));
        er.set_dequeue_pointer(GuestAddress(0x100));

        let mut trb = Trb::new();

        // Fill first table.
        trb.set_control(1);
        assert_eq!(er.is_empty(), Ok(true));
        assert_eq!(er.is_full(), Ok(false));
        assert_eq!(er.add_event(trb.clone()), Ok(()));
        assert_eq!(er.is_full(), Ok(false));
        assert_eq!(er.is_empty(), Ok(false));
        let t: Trb = gm.read_obj_from_addr(GuestAddress(0x100)).unwrap();
        assert_eq!(t.get_control(), 1);
        assert_eq!(t.get_cycle(), 1);

        trb.set_control(2);
        assert_eq!(er.add_event(trb.clone()), Ok(()));
        assert_eq!(er.is_full(), Ok(false));
        assert_eq!(er.is_empty(), Ok(false));
        let t: Trb = gm
            .read_obj_from_addr(GuestAddress(0x100 + trb_size))
            .unwrap();
        assert_eq!(t.get_control(), 2);
        assert_eq!(t.get_cycle(), 1);

        trb.set_control(3);
        assert_eq!(er.add_event(trb.clone()), Ok(()));
        assert_eq!(er.is_full(), Ok(false));
        assert_eq!(er.is_empty(), Ok(false));
        let t: Trb = gm
            .read_obj_from_addr(GuestAddress(0x100 + 2 * trb_size))
            .unwrap();
        assert_eq!(t.get_control(), 3);
        assert_eq!(t.get_cycle(), 1);

        // Fill second table.
        trb.set_control(4);
        assert_eq!(er.add_event(trb.clone()), Ok(()));
        assert_eq!(er.is_full(), Ok(false));
        assert_eq!(er.is_empty(), Ok(false));
        let t: Trb = gm.read_obj_from_addr(GuestAddress(0x200)).unwrap();
        assert_eq!(t.get_control(), 4);
        assert_eq!(t.get_cycle(), 1);

        trb.set_control(5);
        assert_eq!(er.add_event(trb.clone()), Ok(()));
        assert_eq!(er.is_full(), Ok(false));
        assert_eq!(er.is_empty(), Ok(false));
        let t: Trb = gm
            .read_obj_from_addr(GuestAddress(0x200 + trb_size))
            .unwrap();
        assert_eq!(t.get_control(), 5);
        assert_eq!(t.get_cycle(), 1);

        trb.set_control(6);
        assert_eq!(er.add_event(trb.clone()), Ok(()));
        assert_eq!(er.is_full(), Ok(false));
        assert_eq!(er.is_empty(), Ok(false));
        let t: Trb = gm
            .read_obj_from_addr(GuestAddress(0x200 + 2 * trb_size as u64))
            .unwrap();
        assert_eq!(t.get_control(), 6);
        assert_eq!(t.get_cycle(), 1);

        // Fill third table.
        trb.set_control(7);
        assert_eq!(er.add_event(trb.clone()), Ok(()));
        assert_eq!(er.is_full(), Ok(false));
        assert_eq!(er.is_empty(), Ok(false));
        let t: Trb = gm.read_obj_from_addr(GuestAddress(0x300)).unwrap();
        assert_eq!(t.get_control(), 7);
        assert_eq!(t.get_cycle(), 1);

        trb.set_control(8);
        assert_eq!(er.add_event(trb.clone()), Ok(()));
        // There is only one last trb. Considered full.
        assert_eq!(er.is_full(), Ok(true));
        assert_eq!(er.is_empty(), Ok(false));
        let t: Trb = gm
            .read_obj_from_addr(GuestAddress(0x300 + trb_size))
            .unwrap();
        assert_eq!(t.get_control(), 8);
        assert_eq!(t.get_cycle(), 1);

        // Add the last trb will result in error.
        assert_eq!(er.add_event(trb.clone()), Err(Error::EventRingFull));

        // Dequeue one trb.
        er.set_dequeue_pointer(GuestAddress(0x100 + trb_size));
        assert_eq!(er.is_full(), Ok(false));
        assert_eq!(er.is_empty(), Ok(false));

        // Fill the last trb of the third table.
        trb.set_control(9);
        assert_eq!(er.add_event(trb.clone()), Ok(()));
        // There is only one last trb. Considered full.
        assert_eq!(er.is_full(), Ok(true));
        assert_eq!(er.is_empty(), Ok(false));
        let t: Trb = gm
            .read_obj_from_addr(GuestAddress(0x300 + trb_size))
            .unwrap();
        assert_eq!(t.get_control(), 8);
        assert_eq!(t.get_cycle(), 1);

        // Add the last trb will result in error.
        assert_eq!(er.add_event(trb.clone()), Err(Error::EventRingFull));

        // Dequeue until empty.
        er.set_dequeue_pointer(GuestAddress(0x100));
        assert_eq!(er.is_full(), Ok(false));
        assert_eq!(er.is_empty(), Ok(true));

        // Fill first table again.
        trb.set_control(10);
        assert_eq!(er.add_event(trb.clone()), Ok(()));
        assert_eq!(er.is_full(), Ok(false));
        assert_eq!(er.is_empty(), Ok(false));
        let t: Trb = gm.read_obj_from_addr(GuestAddress(0x100)).unwrap();
        assert_eq!(t.get_control(), 10);
        // cycle bit should be reversed.
        assert_eq!(t.get_cycle(), 0);

        trb.set_control(11);
        assert_eq!(er.add_event(trb.clone()), Ok(()));
        assert_eq!(er.is_full(), Ok(false));
        assert_eq!(er.is_empty(), Ok(false));
        let t: Trb = gm
            .read_obj_from_addr(GuestAddress(0x100 + trb_size))
            .unwrap();
        assert_eq!(t.get_control(), 11);
        assert_eq!(t.get_cycle(), 0);

        trb.set_control(12);
        assert_eq!(er.add_event(trb.clone()), Ok(()));
        assert_eq!(er.is_full(), Ok(false));
        assert_eq!(er.is_empty(), Ok(false));
        let t: Trb = gm
            .read_obj_from_addr(GuestAddress(0x100 + 2 * trb_size))
            .unwrap();
        assert_eq!(t.get_control(), 12);
        assert_eq!(t.get_cycle(), 0);
    }
}