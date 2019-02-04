// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std;
use std::os::raw::{c_short, c_void};
use std::os::unix::io::RawFd;

use bindings;
use libusb_device::LibUsbDevice;

pub enum HotPlugEvent {
    DeviceArrived,
    DeviceLeft,
}

impl HotPlugEvent {
    pub fn new(event: bindings::libusb_hotplug_event) -> Self {
        match event {
            bindings::LIBUSB_HOTPLUG_EVENT_DEVICE_ARRIVED => HotPlugEvent::DeviceArrived,
            bindings::LIBUSB_HOTPLUG_EVENT_DEVICE_LEFT => HotPlugEvent::DeviceLeft,
        }
    }
}

pub trait UsbHotplugHandler: Send + Sync + 'static {
    fn hotplug_event(device: LibUsbDevice, event: HotPlugEvent);
}

struct UsbHotplugHandlerHolder {
    context: Arc<LibUsbContextInner>,
    handler: Box<LibUsbPollfdChangeHandler>,
}

impl UsbHotplugHandlerHolder {
    pub fn new<H: UsbHotplugHandler>(context: Arc<LibUsbContextInner>, handler: UsbHotplugHandler) -> Box<UsbHotplugHandlerHolder> {
        let holder = UsbHotplugHandlerHolder {
            context,
            handler: Box::new(handler),
        };
        Box::new(holder)
    }

}

// This function is safe when user_data points to valid PollfdChangeHandlerHolder.
pub unsafe extern "C" fn hotplug_cb(ctx: *mut bindings::libusb_context,
        device: *mut bindings::libusb_device,
        event: bindings::libusb_hotplug_event,
        user_data: *mut c_void) {
    // Safe because user_data was casted from holder.
    let holder = &*(user_data as *mut UsbHotplugHandlerHolder);
    let device = LibUsbDevice::new(
        holder.context.clone(),
        device,
    );
    let event = HotPlugEvent::new(event);
    keeper.handler.hotplug_event(device, event);
}