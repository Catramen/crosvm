// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::xhci_backend_device::XhciBackendDevice;
use super::usb_hub::UsbHub;
use usb::event_loop::EventLoop;
use std::sync::{Arc, Mutex};
use std::os::unix::io::RawFd;

/// Xhci backend provider will run on an EventLoop and connect new devices to usb ports.
pub trait XhciBackendDeviceProvider: Send {
    /// Start the provider on EventLoop.
    fn start(&mut self, event_loop: EventLoop, hub: Arc<UsbHub>);

    // Keep fds that should keep open.
    fn keep_fds(&self) -> RawFd;
}
