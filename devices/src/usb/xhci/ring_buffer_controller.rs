// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use usb::event_loop::{EventLoop, EventHandler};
use usb::xhci::xhci_abi::*;
use std::sync::{
    Arc,
    Weak,
    Mutex,
};

use sys_util::{
    EventFd,
    PollContext,
    WatchingEvents,
    GuestAddress,
    GuestMemory
};

use super::ring_buffer::RingBuffer;

type TransferDescriptor = Vec<AddressedTrb>;

enum RingBufferState {
    Running,
    Stopping,
    Stopped,
}

pub trait TransferDescriptorHandler {
    /// Process descriptor asynchronously, write complete_event when finishes.
    fn handle_transfer_descriptor(&self, descriptor: TransferDescriptor, complete_event: EventFd);
}

pub struct RingBufferController<T: 'static + TransferDescriptorHandler> {
    state: Mutex<RingBufferState>,
    stop_callback: Mutex<Vec<AutoCallback>>,
    ring_buffer: Mutex<RingBuffer>,
    handler: T,
    event_loop: EventLoop,
    event: EventFd,
}

impl<T> RingBufferController<T> where T: 'static + TransferDescriptorHandler {
    /// Create a ring buffer controller and add it to event loop.
    pub fn create_controller(mem: GuestMemory, event_loop: &EventLoop, handler: T)
        -> Arc<RingBufferController<T>> {
        let evt = EventFd::new().unwrap();
        let rawfd = EventFd::as_raw_fd(&evt);
        let controller = Arc::new(
                RingBufferController {
                    ring_buffer: Mutex::new(RingBuffer::new(mem)),
                    handler,
                    event_loop: event_loop.clone(),
                    event: evt,
                }
            );
        let event_handler: Arc<EventHandler> = controller.clone();
        event_loop.add_event(rawfd,
                             WatchingEvents::empty().set_read(),
                             Arc::downgrade(&event_handler));
        controller
    }

    pub fn set_dequeue_pointer(&self, ptr: GuestAddress) {
        // Fast because this should only hanppen during xhci setup.
        self.ring_buffer.lock().unwrap().set_dequeue_pointer(ptr);
    }

    pub fn set_consumer_cycle_state(&self, state: bool) {
        // Fast because this should only hanppen during xhci setup.
        self.ring_buffer.lock().unwrap().set_consumer_cycle_state(state);
    }

    pub fn start(&self) {
        let mut state = self.state.lock().unwrap();
        if *state != RingBufferState::Running {
            *state = RingBufferState::Running;
            self.event.write(1).unwrap();
        }
    }

    pub fn stop(&self, callback: AutoCallback) {
        let mut state = self.state.lock().unwrap();
        if *state == RingBufferState::Stopped {
            return;
        }
        *state = RingBufferState::Stopping;
        self.stop_callback.lock().unwrap().push(callback);
    }
}

impl<T> Drop for RingBufferController<T> where T: 'static + TransferDescriptorHandler {
    fn drop(&mut self) {
        /// Remove self from the event loop.
        self.event_loop.remove_event_for_fd(EventFd::as_raw_fd(&self.event));
    }
}


impl<T> EventHandler for RingBufferController<T> where T: 'static + TransferDescriptorHandler {
    fn on_event(&self, _fd: RawFd) {
        let _ = self.event.read();
        let transfer_descriptor = {
            let mut ring_buffer = self.ring_buffer.lock().unwrap();
            ring_buffer.dequeue_transfer_descriptor()
        };

        let transfer_descriptor = {
            let mut state = self.state.lock().unwrap();
            if *state == RingBufferState::Stopped {
                return;
            } else if *state == RingBuffer::Stopping || transfer_descriptor.is_none() {
                *state = RingBufferState::Stopped;
                self.stop_callback.lock().unwrap().clear();
                return;
            }
            transfer_descriptor.unwrap()
        };

        let event = self.event.try_clone().unwrap();
        self.handler.handle_transfer_descriptor(transfer_descriptor, event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;
    use std::sync::mpsc::{channel, Sender, Receiver};

    struct TestHandler {
        sender: Sender<i32>,
    }

    impl TransferDescriptorHandler for TestHandler {
        fn handle_transfer_descriptor(&self,
                                      descriptor: TransferDescriptor,
                                      complete_event: EventFd) {
            for atrb in descriptor {
                assert_eq!(atrb.trb.trb_type().unwrap(), TrbType::Normal);
                self.sender.send(atrb.trb.get_parameter() as i32).unwrap();
            }
        }
    }

    fn setup_mem() -> GuestMemory {
        let trb_size = size_of::<Trb>() as u64;
        let gm = GuestMemory::new(&vec![(GuestAddress(0), 0x1000)]).unwrap();

        // Structure of ring buffer:
        //  0x100  --> 0x200  --> 0x300
        //  trb 1  |   trb 3  |   trb 5
        //  trb 2  |   trb 4  |   trb 6
        //  l trb  -   l trb  -   l trb to 0x100
        let mut trb = NormalTrb::new();
        trb.set_trb_type(TrbType::Normal as u8);
        trb.set_data_buffer(1);
        trb.set_chain(1);
        gm.write_obj_at_addr(trb.clone(), GuestAddress(0x100)).unwrap();

        trb.set_data_buffer(2);
        gm.write_obj_at_addr(trb, GuestAddress(0x100 + trb_size)).unwrap();

        let mut ltrb = LinkTrb::new();
        ltrb.set_trb_type(TrbType::Link as u8);
        ltrb.set_ring_segment_pointer(0x200);
        gm.write_obj_at_addr(ltrb, GuestAddress(0x100 + 2 * trb_size)).unwrap();

        trb.set_data_buffer(3);
        gm.write_obj_at_addr(trb, GuestAddress(0x200)).unwrap();

        // Chain bit is false.
        trb.set_data_buffer(4);
        trb.set_chain(0);
        gm.write_obj_at_addr(trb, GuestAddress(0x200 + 1 * trb_size)).unwrap();

        ltrb.set_ring_segment_pointer(0x300);
        gm.write_obj_at_addr(ltrb, GuestAddress(0x200 + 2 * trb_size)).unwrap();

        trb.set_data_buffer(5);
        trb.set_chain(1);
        gm.write_obj_at_addr(trb, GuestAddress(0x300)).unwrap();

        // Chain bit is false.
        trb.set_data_buffer(6);
        trb.set_chain(0);
        gm.write_obj_at_addr(trb, GuestAddress(0x300 + 1 * trb_size)).unwrap();

        ltrb.set_ring_segment_pointer(0x100);
        gm.write_obj_at_addr(ltrb, GuestAddress(0x300 + 2 * trb_size)).unwrap();
        gm
    }

    #[test]
    fn test_ring_buffer_controller() {
        let (tx, rx) = channel();
        let mem = setup_mem();
        let (l,j) = EventLoop::start();
        let controller = RingBufferController::create_controller(mem,
                                                                 &l,
                                                                 TestHandler {
                                                                     sender: tx,
                                                                 });
        controller.set_dequeue_pointer(GuestAddress(0x100));
        controller.set_consumer_cycle_state(false);
        controller.start();
        assert_eq!(rx.recv().unwrap(), 1);
        assert_eq!(rx.recv().unwrap(), 2);
        assert_eq!(rx.recv().unwrap(), 3);
        assert_eq!(rx.recv().unwrap(), 4);
        controller.start();
        assert_eq!(rx.recv().unwrap(), 5);
        assert_eq!(rx.recv().unwrap(), 6);
        l.stop();
        j.join().unwrap();
    }
}
