// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::boxed::Box;
use std::sync::Arc;

use usb::libusb::bindings::*;
use usb::libusb::error::*;
use usb::libusb::device_handle::*;

pub trait TransferBuffer {
    fn raw_buffer(&self) -> *mut u8;
    fn buffer_len(&self) -> i32;
}

struct TransferCallbackData <T: TransferBuffer> {
    callback: Box<Fn(LibUsbTransfer)>,
    transfer: LibUsbTransfer,
    buffer: T,
}

impl<T> TransferCallbackData<T> {
    pub fn TransferCompletionCallback(transfer: *mut libusb_transfer) {
        let data_raw = unsafe {
            transfer.user_data as *mut TransferCallbackData
        };
        let callback_data = Box::from_raw(data_raw);
        callback_data.callback(callback_data.transfer);
    }
}

#[derive(Clone)]
pub struct LibUsbTransfer(Arc<LibUsbTransferImpl>);

pub struct LibUsbTransferImpl {
    transfer: *mut libusb_transfer,
}

impl Drop for LibUsbTransferImpl {
    fn drop(&mut self) {
        unsafe {
            libusb_free_transfer(self.transfer);
        }
    }
}

impl LibUsbTransfer {
    // Libusb asynchronous I/O interface has a 5 step process. It gives lots of
    // flexibility but makes it hard to manage object life cycle and easy to
    // write unsafe code. We wrap this interface to a simple "transfer" and "cancel"
    // interface. Resubmission is not allowed and deallocation is handled safely
    // here.
    pub fn asyn_bulk_transfer<T: TransferBuffer>(handle: DeviceHandle, endpoint: u8, buffer: T,
                        callback: Box<Fn(LibUsbTransfer)>, timeout: u32) -> Result<LibUsbTransfer> {
        let mut transfer: *mut libusb_transfer = unsafe {
            transfer = libusb_alloc_transfer(0);
        };
        let transfer = LibUsbTransfer(Arc::new(LibUsbTransferImpl { transfer: transfer })));
        let raw_buffer = buffer.raw_buffer();
        let buffer_len = buffer.buffer_len();
        let callback_data = Box::new(TransferCallbackData {
            callback: callback,
            transfer: transfer.clone(),
            buffer: buffer,
        });
        let userdata = callback_data.into_raw();
        unsafe {
            libusb_fill_bulk_transfer(transfer.0.transfer, handle.handle, endpoint,
                                         raw_buffer, buffer_len, TransferCallbackData::TransferCompletionCallback,
                                         userdata, timeout);
        }

        call_libusb_fn!(libusb_submit_transfer(self.transfer));
        Ok(new_transfer)
    }

    pub fn asyn_control_transfer<T: TransferBuffer>(handle: DeviceHandle, buffer: T,
                        callback: Box<Fn(LibUsbTransfer)>, timeout: u32) -> Result<LibUsbTransfer> {
        let mut transfer: *mut libusb_transfer = unsafe {
            transfer = libusb_alloc_transfer(0);
        };
        let transfer = LibUsbTransfer(Arc::new(LibUsbTransferImpl { transfer: transfer })));
        let raw_buffer = buffer.raw_buffer();
        let callback_data = Box::new(TransferCallbackData {
            callback: callback,
            transfer: transfer.clone(),
            buffer: buffer,
        });
        let userdata = callback_data.into_raw();
        unsafe {
            libusb_fill_control_transfer(transfer.0.transfer, handle.handle, endpoint,
                                         raw_buffer, TransferCallbackData::TransferCompletionCallback,
                                         userdata, timeout);
        }

        call_libusb_fn!(libusb_submit_transfer(self.transfer));
        Ok(new_transfer)
    }

    fn cancel(&self) -> Result<()> {
        call_libusb_fn!(libusb_cancel_transfer(self.0.transfer));
        Ok(())
    }
}

