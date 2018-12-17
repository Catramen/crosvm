// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::interrupter::Interrupter;
use std::os::unix::io::RawFd;
use std::sync::Arc;
use sync::Mutex;
use sys_util::{EventFd, WatchingEvents};
use usb::error::{Error, Result};
use usb::event_loop::{EventHandler, EventLoop};

/// Interrupt Resample handler handles resample event. It will reassert interrupt if needed.
pub struct IntrResampleHandler {
    interrupter: Arc<Mutex<Interrupter>>,
    resample_evt: EventFd,
    irq_evt: EventFd,
}

impl IntrResampleHandler {
    /// Start resample handler.
    pub fn start(
        event_loop: &EventLoop,
        interrupter: Arc<Mutex<Interrupter>>,
        resample_evt: EventFd,
        irq_evt: EventFd,
    ) -> Arc<IntrResampleHandler> {
        let handler = Arc::new(IntrResampleHandler {
            interrupter,
            resample_evt,
            irq_evt,
        });
        let tmp_handler: Arc<EventHandler> = handler.clone();
        event_loop.add_event(
            &handler.resample_evt,
            WatchingEvents::empty().set_read(),
            Arc::downgrade(&tmp_handler),
        );
        handler
    }
}
impl EventHandler for IntrResampleHandler {
    fn on_event(&self, _fd: RawFd) -> Result<()> {
        // This read should not fail. The supplied buffer is correct and eventfd is not nonblocking
        // here. See eventfd(2) for more details. We are not interested in the read value, instead
        // of the fact that there is an event.
        let _ = self.resample_evt.read().map_err(err_msg!(Error::SysError));
        debug!("resample triggered");
        if !self.interrupter.lock().event_ring_is_empty() {
            debug!("irq resample re-assert irq event");
            // There could be a race condition. When we get resample_evt and other
            // component is sending interrupt at the same time.
            // This might result in one more interrupt than we want. It's handled by
            // kernel correctly.
            self.irq_evt.write(1).map_err(err_msg!(Error::SysError))?;
        }
        Ok(())
    }
}
