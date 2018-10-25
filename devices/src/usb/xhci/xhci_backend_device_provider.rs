// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::usb_hub::UsbHub;
use std::os::unix::io::RawFd;
use std::sync::Arc;
use usb::event_loop::EventLoop;

/// Xhci backend provider will run on an EventLoop and connect new devices to usb ports.
pub trait XhciBackendDeviceProvider: Send {
    /// Start the provider on EventLoop.
    fn start(&mut self, event_loop: Arc<EventLoop>, hub: Arc<UsbHub>);

    /// Keep fds that should be kept open.
    fn keep_fds(&self) -> Vec<RawFd>;
}
