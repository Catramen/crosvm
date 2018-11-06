// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::interrupter::Interrupter;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::sync::{Arc, Mutex, Weak};
use sys_util::{EventFd, GuestAddress, GuestMemory, PollContext, WatchingEvents};
use usb::event_loop::{EventLoop, EventHandler};

pub struct IntrResampleHandler {
    interrupter: Arc<Mutex<Interrupter>>,
    resample_evt: EventFd,
    irq_evt: EventFd,
}

impl IntrResampleHandler {
    pub fn start(
        event_loop: &EventLoop,
        interrupter: Arc<Mutex<Interrupter>>,
        resample_evt: EventFd,
        irq_evt: EventFd,
    ) -> Arc<IntrResampleHandler> {
        let rawfd = EventFd::as_raw_fd(&resample_evt);
        let handler = Arc::new(IntrResampleHandler {
            interrupter,
            resample_evt,
            irq_evt,
        });
        let tmp_handler: Arc<EventHandler> = handler.clone();
        debug!("event loop add event {} - IntrResampleHandler", rawfd);
        event_loop.add_event(
            rawfd,
            WatchingEvents::empty().set_read(),
            Arc::downgrade(&tmp_handler),
        );
        handler
    }
}
impl EventHandler for IntrResampleHandler {
    fn on_event(&self, _fd: RawFd) {
        let _ = self.resample_evt.read();
        debug!("resample triggered");
        if self.interrupter.lock().unwrap().er_not_empty() {
            debug!("irq resample re-assert irq event");
            // There could be a race condition. When we get resample_evt and other
            // component is sending interrupt at the same time.
            // This might result in one more interrupt than we want. It's handled by
            // kernel correctly.
            self.irq_evt.write(1);
        }
    }
}
