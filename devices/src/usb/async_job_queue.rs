// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use usb::event_loop::{EventLoop, EventHandler};
use std::sync::{Arc, Weak, Mutex};
use std::os::unix::io::{RawFd, AsRawFd};
use sys_util::{WatchingEvents, EventFd};
use std::mem::swap;

/// Async Job Queue can schedule async jobs.
pub struct AsyncJobQueue {
    jobs: Mutex<Vec<Box<Fn() + 'static + Send>>>,
    evt: EventFd,
}

impl AsyncJobQueue {
    pub fn init(event_loop: &EventLoop) -> Arc<AsyncJobQueue> {
        let evt = EventFd::new().unwrap();
        let fd = evt.as_raw_fd();
        let queue = Arc::new(
            AsyncJobQueue {
                jobs: Mutex::new(Vec::new()),
                evt,
            }
        );
        let handler: Arc<EventHandler> = queue.clone();
        event_loop.add_event(fd,
                             WatchingEvents::empty().set_read(),
                             Arc::downgrade(&handler)
        );
        queue
    }
    pub fn queue_job<T: Fn() + 'static + Send>(&self, cb: T) {
        self.jobs.lock().unwrap().push(
            Box::new(cb)
        );
        self.evt.write(1).unwrap();
    }
}

impl EventHandler for AsyncJobQueue {
    fn on_event(&self, _fd: RawFd) {
        let _ = self.evt.read();
        let mut jobs = Vec::new();
        std::mem::swap(&mut jobs, &mut *self.jobs.lock().unwrap());
        for cb in jobs {
            cb();
        }
    }
}
