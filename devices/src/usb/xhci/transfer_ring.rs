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
}

impl TransferRing {
    fn get_next_trb(&self) {
    }
}

