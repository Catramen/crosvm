// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::os::unix::io::RawFd;
use std::sync::{Arc, Mutex, MutexGuard};
use usb::auto_callback::AutoCallback;
use usb::event_loop::{EventHandler, EventLoop};
use usb::xhci::xhci_abi::*;

use sys_util::{EventFd, GuestAddress, GuestMemory, WatchingEvents};

use super::ring_buffer::RingBuffer;

#[derive(PartialEq, Copy, Clone)]
enum RingBufferState {
    /// Running: RingBuffer is running, consuming transfer descriptor.
    Running,
    /// Stopping: Some thread requested RingBuffer stop. It will stop when current descriptor is
    /// handled.
    Stopping,
    /// Stopped: RingBuffer already stopped.
    Stopped,
}

/// TransferDescriptorHandler handles transfer descriptor. User should implement this trait and build
/// a ring buffer controller with the struct.
pub trait TransferDescriptorHandler {
    /// Process descriptor asynchronously, write complete_event when done.
    fn handle_transfer_descriptor(&self, descriptor: TransferDescriptor, complete_event: EventFd);
    /// Stop is called when trying to stop ring buffer controller. Returns true when stop must be
    /// performed asynchronously. This happens because the handler is handling some descriptor
    /// asynchronously, the stop callback of ring buffer controller must be called after the
    /// `async` part is handled or cancelled. If the TransferDescriptorHandler decide it could stop
    /// immediately, it could return false.
    /// For example, if a handler submitted a transfer but the transfer has not yet finished. Then
    /// guest kernel requests to stop the ring buffer controller. Transfer descriptor handler will
    /// return true, thus RingBufferController would transfer to Stopping state. It will be stopped
    /// when all pending transfer completed.
    /// On the other hand, if hander does not have any pending transfers, it would return false.
    fn stop(&self) -> bool {
        true
    }
}

/// RingBufferController owns a ring buffer. It lives on a event_loop. It will pop out transfer
/// descriptor and let TransferDescriptorHandler handle it.
pub struct RingBufferController<T: 'static + TransferDescriptorHandler> {
    name: String,
    state: Mutex<RingBufferState>,
    stop_callback: Mutex<Vec<AutoCallback>>,
    ring_buffer: Mutex<RingBuffer>,
    handler: Mutex<T>,
    event_loop: Arc<EventLoop>,
    event: EventFd,
}

impl<T: Send> RingBufferController<T>
where
    T: 'static + TransferDescriptorHandler,
{
    /// Create a ring buffer controller and add it to event loop.
    pub fn create_controller(
        name: String,
        mem: GuestMemory,
        event_loop: Arc<EventLoop>,
        handler: T,
    ) -> Arc<RingBufferController<T>> {
        let evt = EventFd::new().expect("Could not create event fd");
        let controller = Arc::new(RingBufferController {
            name: name.clone(),
            state: Mutex::new(RingBufferState::Stopped),
            stop_callback: Mutex::new(Vec::new()),
            ring_buffer: Mutex::new(RingBuffer::new(name.clone(), mem)),
            handler: Mutex::new(handler),
            event_loop: event_loop.clone(),
            event: evt,
        });
        let event_handler: Arc<EventHandler> = controller.clone();
        event_loop.add_event(
            &controller.event,
            WatchingEvents::empty().set_read(),
            Arc::downgrade(&event_handler),
        );
        controller
    }

    fn lock_ring_buffer(&self) -> MutexGuard<RingBuffer> {
        self.ring_buffer.lock().expect("cannot lock ring buffer")
    }

    /// Set dequeue pointer of the internal ring buffer.
    pub fn set_dequeue_pointer(&self, ptr: GuestAddress) {
        debug!("{}: set dequeue pointer: {:x}", self.name, ptr.0);
        // Fast because this should only hanppen during xhci setup.
        self.lock_ring_buffer().set_dequeue_pointer(ptr);
    }

    /// Set consumer cycle state.
    pub fn set_consumer_cycle_state(&self, state: bool) {
        debug!("{}: set consumer cycle state: {}", self.name, state);
        // Fast because this should only hanppen during xhci setup.
        self.lock_ring_buffer().set_consumer_cycle_state(state);
    }

    /// Start the ring buffer.
    pub fn start(&self) {
        debug!("{} started", self.name);
        let mut state = self.state.lock().unwrap();
        if *state != RingBufferState::Running {
            *state = RingBufferState::Running;
            self.event.write(1).expect("cannot write to event fd");
        }
    }

    /// Stop the ring buffer asynchronously.
    pub fn stop(&self, callback: AutoCallback) {
        debug!("{} being stopped", self.name);
        let mut state = self.state.lock().unwrap();
        if *state == RingBufferState::Stopped {
            debug!("{} is already stopped", self.name);
            return;
        }
        if self.handler.lock().unwrap().stop() {
            *state = RingBufferState::Stopping;
            self.stop_callback.lock().unwrap().push(callback);
        } else {
            *state = RingBufferState::Stopped;
        }
    }
}

impl<T> Drop for RingBufferController<T>
where
    T: 'static + TransferDescriptorHandler,
{
    fn drop(&mut self) {
        // Remove self from the event loop.
        self.event_loop.remove_event_for_fd(&self.event);
    }
}

impl<T> EventHandler for RingBufferController<T>
where
    T: 'static + TransferDescriptorHandler + Send,
{
    fn on_event(&self, _fd: RawFd) {
        // `self.event` triggers ring buffer controller to run, the value read is not important.
        let _ = self.event.read();
        let mut state = self.state.lock().unwrap();

        match *state {
            RingBufferState::Stopped => return,
            RingBufferState::Stopping => {
                debug!("{}: stopping ring buffer controller", self.name);
                *state = RingBufferState::Stopped;
                self.stop_callback.lock().unwrap().clear();
                return;
            }
            RingBufferState::Running => {}
        }

        let transfer_descriptor = match self.lock_ring_buffer().dequeue_transfer_descriptor() {
            Some(transfer_descriptor) => transfer_descriptor,
            None => {
                *state = RingBufferState::Stopped;
                self.stop_callback.lock().unwrap().clear();
                return;
            }
        };

        let event = self.event.try_clone().unwrap();
        self.handler
            .lock()
            .unwrap()
            .handle_transfer_descriptor(transfer_descriptor, event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;
    use std::sync::mpsc::{channel, Sender};

    struct TestHandler {
        sender: Sender<i32>,
    }

    impl TransferDescriptorHandler for TestHandler {
        fn handle_transfer_descriptor(
            &self,
            descriptor: TransferDescriptor,
            complete_event: EventFd,
        ) {
            for atrb in descriptor {
                assert_eq!(atrb.trb.trb_type().unwrap(), TrbType::Normal);
                self.sender.send(atrb.trb.get_parameter() as i32).unwrap();
            }
            complete_event.write(1).unwrap();
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
        gm.write_obj_at_addr(trb.clone(), GuestAddress(0x100))
            .unwrap();

        trb.set_data_buffer(2);
        gm.write_obj_at_addr(trb, GuestAddress(0x100 + trb_size))
            .unwrap();

        let mut ltrb = LinkTrb::new();
        ltrb.set_trb_type(TrbType::Link as u8);
        ltrb.set_ring_segment_pointer(0x200);
        gm.write_obj_at_addr(ltrb, GuestAddress(0x100 + 2 * trb_size))
            .unwrap();

        trb.set_data_buffer(3);
        gm.write_obj_at_addr(trb, GuestAddress(0x200)).unwrap();

        // Chain bit is false.
        trb.set_data_buffer(4);
        trb.set_chain(0);
        gm.write_obj_at_addr(trb, GuestAddress(0x200 + 1 * trb_size))
            .unwrap();

        ltrb.set_ring_segment_pointer(0x300);
        gm.write_obj_at_addr(ltrb, GuestAddress(0x200 + 2 * trb_size))
            .unwrap();

        trb.set_data_buffer(5);
        trb.set_chain(1);
        gm.write_obj_at_addr(trb, GuestAddress(0x300)).unwrap();

        // Chain bit is false.
        trb.set_data_buffer(6);
        trb.set_chain(0);
        gm.write_obj_at_addr(trb, GuestAddress(0x300 + 1 * trb_size))
            .unwrap();

        ltrb.set_ring_segment_pointer(0x100);
        gm.write_obj_at_addr(ltrb, GuestAddress(0x300 + 2 * trb_size))
            .unwrap();
        gm
    }

    #[test]
    fn test_ring_buffer_controller() {
        let (tx, rx) = channel();
        let mem = setup_mem();
        let (l, j) = EventLoop::start();
        let l = Arc::new(l);
        let controller = RingBufferController::create_controller(
            "".to_string(),
            mem,
            l.clone(),
            TestHandler { sender: tx },
        );
        controller.set_dequeue_pointer(GuestAddress(0x100));
        controller.set_consumer_cycle_state(false);
        controller.start();
        assert_eq!(rx.recv().unwrap(), 1);
        assert_eq!(rx.recv().unwrap(), 2);
        assert_eq!(rx.recv().unwrap(), 3);
        assert_eq!(rx.recv().unwrap(), 4);
        assert_eq!(rx.recv().unwrap(), 5);
        assert_eq!(rx.recv().unwrap(), 6);
        l.stop();
        j.join().unwrap();
    }
}
