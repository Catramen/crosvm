// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// Transfer Ring is segmented circular buffer in guest memory containing work items
// called transfer descriptors, each of which consists of one or more TRBs.
// Transfer Ring management is defined in 4.9.2.
pub struct TransferRing {
    mem: &GuestMemory,
    dequeue_pointer: GuestAddress,
    // Used to check if the ring is empty. Toggled when looping back to the begining
    // of the buffer.
    consumer_cycle_state: bool,
}

// Public interfaces for Transfer Ring.
impl TransferRing {
    pub fn new(mem: &GuestMemory) -> Self {
        TransferRing {
            mem: mem,
            dequeue_pointer: GuestAddress(0),
            consumer_cycle_state: true,
        }
    }

    pub fn dequeue_transfer_descriptor(&mut self) -> Option<Vec<Trb>> {
        let td: Vec<Trb> = Vec::new();
        loop {
            let addressed_trb = match self.get_next_trb() {
                Some(t) => t,
                None => break,
            }

            if addressed_trb.trb.get_trb_type() == TrbType::Link {
                let link_trb = addressed_trb.trb.cast::<LinkTrb>();
                self.dequeue_pointer = GuestAddress(link_trb.ring_segment_pointer);
                self.consumer_cycle_state = (self.consumer_cycle_state != link_trb.get_toggle_cycle());
                continue;
            }
            self.dequeue_pointer = match self.dequeue_pointer.checked_add(TRB_SIZE) {
                Some(addr) => addr,
                None => panic!("Crash due to unknown bug"),
            }
            td.push(addressed_trb);
            if !addressed_trb.get_chain_bit() {
                break;
            }
        }
        if td.len() == 0 || td.last().trb.get_chain_bit() {
            None
        }
        td
    }

    pub fn set_dequeue_pointer(&mut self, addr: GuestAddress) {
        self.dequeue_pointer = addr;
    }

    pub fn set_consumer_cycle_state(&mut self, bool state) {
        self.consumer_cycle_state = state;
    }
}

impl TransferRing {
    // Read next trb pointed by dequeue pointer. Does not proceed dequeue pointer.
    fn get_next_trb(&self) -> Option<AddressedTrb> {
        let trb = self.mem.read_obj_from_addr().unwrap();
        // If cycle bit of trb does not equal consumer cycle state, the ring is empty.
        // This trb is invalid.
        if trb.get_cycle() != self.consumer_cycle_state {
            None
        } else {
            Some(AddressedTrb {
                trb: trb,
                gpa: dequeue_pointer.0,
            })
        }
    }
}

