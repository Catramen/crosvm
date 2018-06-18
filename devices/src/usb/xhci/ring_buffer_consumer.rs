// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

pub enum ConsumerState {
    Stopped,
    Stopping,
    Running,
}

pub struct RingBufferConsumer {
    mem: GuestMemory,
    xhci: XHCI,
    state: ConsumerState,
    transfer_ring: TransferRing,
    handler: Box<TransferDescriptorHandler>,
}

pub trait TransferDescriptorHandler {
    fn handle_transfer_descriptor(&self, descriptor: &[AddressedTrb],
                                  callback: Callback);
}

impl RingBufferConsumer {
    // As a consumer, start to handle items in ring.
    pub fn start(&mut self) {
        if (self.state != ConsumerState::Running) {
            self.state = ConsumerState::Running;
            // TODO(jkwang)
            really_start_consuming();
        }
    }

    // Stop the consumer.
    // TODO(jkwang) do we need a callback here?
    pub fn stop(&self) {
        // TODO(jwkang) Details of stop depends thread model.
    }

    // Set the consumer's dequeue pointer.
    pub fn set_dequeue_pointer(&mut self, ptr: GuestAddress) {
        self.transfer_ring.set_dequeue_pointer(ptr);
    }

    // Set the consumer's cycle state bit.
    pub fn set_consumer_cycle_state(&self, state: bool) {
        self.transfer_ring.set_consumer_cycle_state(state);
    }

    pub fn consume_one(&mut self) {
        if self.state == ConsumerState::Stopped {
            return;
        }

        let transfer_descriptor = self.transfer_ring.dequeue_transfer_descriptor();
        if (self.state == ConsumerState::Stopping || transfer_descriptor == None) {
            self.state = ConsumerState::Stopped;
        }

        // What is the callback
        self.handler.handle_transfer_descriptor(transfer_descriptor, some_fk_cb);
    }
}


