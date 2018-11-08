// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::mem;
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::{Arc, Mutex};
use sys_util::{EventFd, WatchingEvents};
use usb::event_loop::{EventHandler, EventLoop};

/// Async Job Queue can schedule async jobs.
pub struct AsyncJobQueue {
    jobs: Mutex<Vec<Box<FnMut() + 'static + Send>>>,
    evt: EventFd,
}

impl AsyncJobQueue {
    /// Init job queue on event loop.
    pub fn init(event_loop: &EventLoop) -> Arc<AsyncJobQueue> {
        let evt = EventFd::new().unwrap();
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
        queue
    }

    pub fn queue_job<T: Fn() + 'static + Send>(&self, cb: T) {
        self.jobs.lock().unwrap().push(Box::new(cb));
        self.evt.write(1).unwrap();
    }
}

impl EventHandler for AsyncJobQueue {
    fn on_event(&self, _fd: RawFd) {
        // We want to read out the event, but the value is not important.
        let _ = self.evt.read();
        let jobs = mem::replace(&mut *self.jobs.lock().unwrap(), Vec::new());
        for mut cb in jobs {
            cb();
        }
    }
}
