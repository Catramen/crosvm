// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::mem;
use std::os::unix::io::RawFd;
use std::sync::{Arc, Mutex};
use sys_util::{EventFd, WatchingEvents};
use usb::error::{Error, Result};
use usb::event_loop::{EventHandler, EventLoop};

/// Async Job Queue can schedule async jobs.
pub struct AsyncJobQueue {
    jobs: Mutex<Vec<Box<FnMut() + 'static + Send>>>,
    evt: EventFd,
}

impl AsyncJobQueue {
    /// Init job queue on event loop.
    pub fn init(event_loop: &EventLoop) -> Result<Arc<AsyncJobQueue>> {
        let evt = EventFd::new().map_err(err_msg!(Error::SysError))?;
        let queue = Arc::new(AsyncJobQueue {
            jobs: Mutex::new(Vec::new()),
            evt,
        });
        let handler: Arc<EventHandler> = queue.clone();
        event_loop.add_event(
            &queue.evt,
            WatchingEvents::empty().set_read(),
            Arc::downgrade(&handler),
        );
        Ok(queue)
    }

    pub fn queue_job<T: Fn() + 'static + Send>(&self, cb: T) -> Result<()> {
        self.jobs
            .lock()
            .map_err(err_msg!(Error::Unknown))?
            .push(Box::new(cb));
        self.evt.write(1).map_err(err_msg!(Error::SysError))
    }
}

impl EventHandler for AsyncJobQueue {
    fn on_event(&self, _fd: RawFd) -> Result<()> {
        // We want to read out the event, but the value is not important.
        let _ = self.evt.read().map_err(err_msg!(Error::SysError))?;
        let jobs = mem::replace(
            &mut *self.jobs.lock().map_err(err_msg!(Error::Unknown))?,
            Vec::new(),
        );
        for mut cb in jobs {
            cb();
        }
        Ok(())
    }
}
