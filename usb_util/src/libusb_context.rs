// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std;
use std::os::raw::{c_short, c_void};
use std::os::unix::io::RawFd;
use std::marker::PhantomData;
use std::slice;

use bindings;
use error::{Result, Error};
use libusb_device::LibUsbDevice;

/// Wrapper for libusb_context. The libusb libary initialization/deinitialization
/// is managed by this context.
/// See: http://libusb.sourceforge.net/api-1.0/group__libusb__lib.html
pub struct LibUsbContext {
    context: *mut bindings::libusb_context,
    pollfd_change_handler: Option<Box<PollfdChangeHandlerHolder>>,
}

impl Drop for LibUsbContext {
    fn drop(&mut self) {
        // Avoid pollfd change handler call when libusb_exit is called.
        self.remove_pollfd_notifiers();
        // Safe beacuse 'self.context' points to a valid context allocated by libusb_init.
        unsafe {
            bindings::libusb_exit(self.context);
        }
    }
}

impl LibUsbContext {
    /// Create a new LibUsbContext.
    pub fn new() -> Result<LibUsbContext> {
        let mut ctx: *mut bindings::libusb_context = std::ptr::null_mut();
        // Safe because '&mut ctx' points to a valid memory (on stack).
        handle_libusb_error!(unsafe {
            bindings::libusb_init(&mut ctx)
        });
        Ok(LibUsbContext { context: ctx, pollfd_change_handler: None })
    }


    /// Returns a list of USB devices currently attached to the system.
    pub fn get_device_iter(&self) -> Result<DeviceIter> {
        let mut list: *mut *mut bindings::libusb_device = std::ptr::null_mut();
        // Safe because 'self.context' points to a valid context and '&mut list' points to a valid
        // memory.
        let n = handle_libusb_error!(unsafe {
            bindings::libusb_get_device_list(self.context, &mut list)
        });

        // Safe because 'list' points to valid memory and n is the length of device structs.
        Ok(
            unsafe {
                DeviceIter {
                    _context: PhantomData,
                    list: slice::from_raw_parts_mut(list, n as usize),
                    index: 0,
                }
            }
          )
    }

    /// Check at runtime if the loaded library has a given capability.
    pub fn has_capability(&self, cap: u32) -> bool {
        // Safe because libusb_init is called before this call happens.
        unsafe { bindings::libusb_has_capability(cap) != 0 }
    }

    /// Return an iter of poll fds. Those fds that should be polled to handle libusb events.
    pub fn get_pollfd_iter(&self) -> PollFdIter {
        // Safe because 'self.context' is inited.
        let list: *mut *const bindings::libusb_pollfd =
            unsafe { bindings::libusb_get_pollfds(self.context) };
        PollFdIter {
            list,
            index: 0,
        }
    }

    /// Handle libusb events in a non block way.
    pub fn handle_events_nonblock(&self) {
        static mut zero_time: bindings::timeval = bindings::timeval {
            tv_sec: 0,
            tv_usec: 0,
        };
        // Safe because 'self.context' points to valid context.
        unsafe {
            bindings::libusb_handle_events_timeout_completed(
                self.context,
                &mut zero_time as *mut bindings::timeval,
                std::ptr::null_mut(),
            );
        }
    }

    /// Set a handler that could handle pollfd change events.
    pub fn set_pollfd_notifiers(&mut self, handler: Box<LibUsbPollfdChangeHandler>) {
        // LibUsbContext is alive when any libusb related function is called. It owns the handler,
        // thus the handler memory is always valid when callback is invoked.
        let holder = Box::new(PollfdChangeHandlerHolder { handler, });
        let raw_holder = Box::into_raw(holder);
        unsafe {
            bindings::libusb_set_pollfd_notifiers(
                self.context,
                Some(pollfd_added_cb),
                Some(pollfd_removed_cb),
                raw_holder as *mut c_void,
            );
        }
        // Safe because raw_holder is from Boxed pointer.
        let holder = unsafe { Box::from_raw(raw_holder) };
        self.pollfd_change_handler = Some(holder);
    }

    /// Remove the previous registered notifiers.
    pub fn remove_pollfd_notifiers(&self) {
        // Safe because 'self.context' is valid.
        unsafe {
            bindings::libusb_set_pollfd_notifiers(self.context, None, None, std::ptr::null_mut());
        }
    }
}

/// Iterator for device list.
pub struct DeviceIter<'a, 'b> {
    _context: PhantomData<&'a LibUsbContext>,
    list: &'b mut [*mut bindings::libusb_device],
    index: usize,
}

impl<'a, 'b> Drop for DeviceIter<'a, 'b> {
    fn drop(&mut self) {
        // Safe because 'self.list' is inited by a valid pointer from libusb_get_device_list.
        unsafe {
            bindings::libusb_free_device_list(self.list.as_mut_ptr(), 1);
        }
    }
}

impl<'a, 'b> Iterator for DeviceIter<'a, 'b> {
    type Item = LibUsbDevice<'a>;

    fn next(&mut self) -> Option<LibUsbDevice<'a>> {
        if self.index >= self.list.len() {
            return None;
        }

        let device = self.list[self.index];
        self.index += 1;
        Some(
            // Safe because 'device' points to a valid memory.
            unsafe {
                LibUsbDevice::new(self._context, device)
            }
            )
    }
}

/// Iterator for pollfds.
pub struct PollFdIter {
    list: *mut *const bindings::libusb_pollfd,
    index: isize,
}

impl Drop for PollFdIter {
    fn drop(&mut self) {
        // Safe because 'self.list' points to valid memory of pollfd list.
        unsafe {
            bindings::libusb_free_pollfds(self.list);
        }
    }
}

impl Iterator for PollFdIter {
    type Item = bindings::libusb_pollfd;

    fn next(&mut self) -> Option<bindings::libusb_pollfd> {
        // Safe because 'self.index' never grow out of the null pointer index.
        let current_ptr = unsafe {
            self.list.offset(self.index)
        };
        if current_ptr.is_null() {
            return None;
        }

        self.index += 1;
        // Safe because 'current_ptr' is not null.
        Some(unsafe {
            (**current_ptr).clone()
        })
    }
}

/// Trait for handler that handles Pollfd Change events.
pub trait LibUsbPollfdChangeHandler {
    fn add_poll_fd(&self, fd: RawFd, events: c_short);
    fn remove_poll_fd(&self, fd: RawFd);
}

// This struct owns LibUsbPollfdChangeHandler. We need it because it's not possible to cast void
// pointer to trait pointer.
struct PollfdChangeHandlerHolder {
    handler: Box<LibUsbPollfdChangeHandler>,
}

extern "C" fn pollfd_added_cb(fd: RawFd, events: c_short, user_data: *mut c_void) {
    // Safe because user_data was casted from hoder.
    let keeper = unsafe { &*(user_data as *mut PollfdChangeHandlerHolder) };
    keeper.handler.add_poll_fd(fd, events);
}

extern "C" fn pollfd_removed_cb(fd: RawFd, user_data: *mut c_void) {
    // Safe because user_data was casted from hoder.
    let keeper = unsafe { &*(user_data as *mut PollfdChangeHandlerHolder) };
    keeper.handler.remove_poll_fd(fd);
}
