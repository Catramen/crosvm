// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::os::raw::c_void;
use std::mem::size_of;

use bindings::{
    libusb_alloc_transfer, libusb_device_handle, libusb_free_transfer, libusb_submit_transfer,
    libusb_transfer, libusb_transfer_status, LIBUSB_TRANSFER_CANCELLED, LIBUSB_TRANSFER_COMPLETED,
    LIBUSB_TRANSFER_ERROR, LIBUSB_TRANSFER_NO_DEVICE, LIBUSB_TRANSFER_OVERFLOW,
    LIBUSB_TRANSFER_STALL, LIBUSB_TRANSFER_TIMED_OUT, LIBUSB_TRANSFER_TYPE_BULK,
    LIBUSB_TRANSFER_TYPE_CONTROL,
};
use error::Error;
use types::UsbRequestSetup;

/// Status of transfer.
pub enum TransferStatus {
    Completed,
    Error,
    TimedOut,
    Cancelled,
    Stall,
    NoDevice,
    OverFlow,
}

impl From<libusb_transfer_status> for TransferStatus {
    fn from(s: libusb_transfer_status) -> Self {
        match s {
            LIBUSB_TRANSFER_COMPLETED => TransferStatus::Completed,
            LIBUSB_TRANSFER_ERROR => TransferStatus::Error,
            LIBUSB_TRANSFER_TIMED_OUT => TransferStatus::TimedOut,
            LIBUSB_TRANSFER_CANCELLED => TransferStatus::Cancelled,
            LIBUSB_TRANSFER_STALL => TransferStatus::Stall,
            LIBUSB_TRANSFER_NO_DEVICE => TransferStatus::NoDevice,
            LIBUSB_TRANSFER_OVERFLOW => TransferStatus::OverFlow,
            _ => TransferStatus::Error,
        }
    }
}

/// Trait for usb transfer buffer.
pub trait UsbTransferBuffer: Send {
    fn as_raw_ptr(&mut self) -> *mut u8;
    fn length(&self) -> i32;
}

/// Default buffer size for control data transfer.
const CONTROL_DATA_BUFFER_SIZE: usize = 1024;

/// Buffer type for control transfer. The first 8-bytes is a UsbRequestSetup struct.
#[repr(C, packed)]
pub struct ControlTransferBuffer {
    pub setup_buffer: UsbRequestSetup,
    pub data_buffer: [u8; CONTROL_DATA_BUFFER_SIZE],
}

impl ControlTransferBuffer {
    fn new() -> ControlTransferBuffer {
        ControlTransferBuffer {
            setup_buffer: UsbRequestSetup {
                request_type: 0,
                request: 0,
                value: 0,
                index: 0,
                length: 0,
            },
            data_buffer: [0; CONTROL_DATA_BUFFER_SIZE],
        }
    }

    pub fn set_request_setup(&mut self, request_setup: &UsbRequestSetup) {
        self.setup_buffer = request_setup.clone();
    }
}

impl UsbTransferBuffer for ControlTransferBuffer {
    fn as_raw_ptr(&mut self) -> *mut u8 {
        self as *mut ControlTransferBuffer as *mut u8
    }

    fn length(&self) -> i32 {
        if self.setup_buffer.length as usize > CONTROL_DATA_BUFFER_SIZE {
            panic!("Setup packet has an oversize length");
        }
        self.setup_buffer.length as i32 + size_of::<UsbRequestSetup>() as i32
    }
}

/// Buffer type for Bulk transfer.
pub struct BulkTransferBuffer {
    buffer: Vec<u8>,
}

impl BulkTransferBuffer {
    fn new(buffer_size: usize) -> Self {
        BulkTransferBuffer {
            buffer: vec![0; buffer_size],
        }
    }

    /// Get mutable interal slice of this buffer.
    pub fn mut_slice(&mut self) -> &mut [u8] {
        &mut self.buffer
    }

    /// Get interal slice of this buffer.
    pub fn slice(&self) -> &[u8] {
        &self.buffer
    }
}

impl UsbTransferBuffer for BulkTransferBuffer {
    fn as_raw_ptr(&mut self) -> *mut u8 {
        &mut (self.buffer[0]) as *mut u8
    }

    fn length(&self) -> i32 {
        self.buffer.len() as i32
    }
}

type UsbTransferCompletionCallback<T> = Fn(UsbTransfer<T>) + Send + 'static;

struct UsbTransferInner<T: UsbTransferBuffer> {
    transfer: *mut libusb_transfer,
    callback: Option<Box<UsbTransferCompletionCallback<T>>>,
    buffer: T,
}

unsafe impl<T: UsbTransferBuffer> Send for UsbTransferInner<T> {}

impl<T: UsbTransferBuffer> Drop for UsbTransferInner<T> {
    fn drop(&mut self) {
        // Safe because 'self.transfer' is valid.
        unsafe {
            libusb_free_transfer(self.transfer);
        }
    }
}

/// UsbTransfer owns a libustbtransfer, it's buffer and callback.
pub struct UsbTransfer<T: UsbTransferBuffer> {
    inner: Box<UsbTransferInner<T>>,
}

/// Build a control transfer.
pub fn control_transfer(timeout: u32) -> UsbTransfer<ControlTransferBuffer> {
    UsbTransfer::<ControlTransferBuffer>::new(
        0,
        LIBUSB_TRANSFER_TYPE_CONTROL as u8,
        timeout,
        ControlTransferBuffer::new(),
    )
}

/// Build a data transfer.
pub fn bulk_transfer(endpoint: u8, timeout: u32, size: usize) -> UsbTransfer<BulkTransferBuffer> {
    UsbTransfer::<BulkTransferBuffer>::new(
        endpoint,
        LIBUSB_TRANSFER_TYPE_BULK as u8,
        timeout,
        BulkTransferBuffer::new(size),
    )
}

impl<T: UsbTransferBuffer> UsbTransfer<T> {
    fn new(endpoint: u8, type_: u8, timeout: u32, buffer: T) -> Self {
        // Safe because alloc is safe.
        let transfer: *mut libusb_transfer = unsafe { libusb_alloc_transfer(0) };
        // Just panic on OOM.
        assert!(!transfer.is_null());
        let inner = Box::new(UsbTransferInner::<T> {
            transfer,
            callback: None,
            buffer,
        });
        // Safe because we inited transfer.
        let raw_transfer: &mut libusb_transfer = unsafe { &mut *(inner.transfer) };
        raw_transfer.endpoint = endpoint;
        raw_transfer.type_ = type_;
        raw_transfer.timeout = timeout;
        raw_transfer.callback = Some(transfer_completion_callback::<T>);
        UsbTransfer { inner }
    }

    /// Set callback function for transfer completion.
    pub fn set_callback<C: 'static + Fn(UsbTransfer<T>) + Send>(&mut self, cb: C) {
        self.inner.callback = Some(Box::new(cb));
    }

    /// Get a reference to the buffer.
    pub fn buffer(&self) -> &T {
        &self.inner.buffer
    }

    /// Get a mutable reference to the buffer.
    pub fn mut_buffer(&mut self) -> &mut T {
        &mut self.inner.buffer
    }

    /// Get actual length of data that was transferred.
    pub fn actual_length(&self) -> i32 {
        let transfer = self.inner.transfer;
        // Safe because inner.transfer is valid memory.
        unsafe { (*transfer).actual_length }
    }

    /// Get the transfer status of this transfer.
    pub fn status(&self) -> TransferStatus {
        let transfer = self.inner.transfer;
        // Safe because inner.transfer is valid memory.
        unsafe { TransferStatus::from((*transfer).status) }
    }

    /// Submit this transfer to device handle. 'self' is consumed. On success, the memory will be
    /// 'leaked' (and store in user_data) and send to libusb, when the async operation is done,
    /// on_transfer_completed will recreate 'self' and deliver it to callback/free 'self'. On
    /// faliure, 'self' is returned with an error.
    pub unsafe fn submit(
        self,
        handle: *mut libusb_device_handle,
    ) -> Result<(), (Error, UsbTransfer<T>)> {
        let transfer = self.into_raw();
        (*transfer).dev_handle = handle;
        match Error::from(libusb_submit_transfer(transfer)) {
            Error::Success(_e) => Ok(()),
            err => Err((err, UsbTransfer::<T>::from_raw(transfer))),
        }
    }

    /// Invoke callback when transfer is completed.
    unsafe fn on_transfer_completed(transfer: *mut libusb_transfer) {
        let mut transfer = UsbTransfer::<T>::from_raw(transfer);
        if transfer.inner.callback.is_none() {
            return;
        }
        // Reset callback to None.
        let cb = transfer.inner.callback.take().unwrap();
        cb(transfer);
    }

    fn into_raw(mut self) -> *mut libusb_transfer {
        let transfer: *mut libusb_transfer = self.inner.transfer;
        // Safe because transfer is valid.
        unsafe {
            (*transfer).buffer = self.mut_buffer().as_raw_ptr();
            (*transfer).length = self.mut_buffer().length();
            (*transfer).user_data = Box::into_raw(self.inner) as *mut c_void;
        }
        transfer
    }

    unsafe fn from_raw(transfer: *mut libusb_transfer) -> Self {
        UsbTransfer {
            inner: Box::<UsbTransferInner<T>>::from_raw(
                (*transfer).user_data as *mut UsbTransferInner<T>,
            ),
        }
    }
}

/// Unsafe code for transfer completion handling.
pub unsafe extern "C" fn transfer_completion_callback<T: UsbTransferBuffer>(
    transfer: *mut libusb_transfer,
) {
    UsbTransfer::<T>::on_transfer_completed(transfer);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn check_control_buffer_size() {
        assert_eq!(
            size_of::<ControlTransferBuffer>(),
            size_of::<UsbRequestSetup>() + CONTROL_DATA_BUFFER_SIZE
        );
    }

    mod test_utils {
        use super::*;
        pub fn fake_submit_transfer<T: UsbTransferBuffer>(transfer: UsbTransfer<T>) {
            let transfer = transfer.into_raw();
            unsafe {
                match (*transfer).callback {
                    Some(cb) => cb(transfer),
                    // Although no callback is invoked, we still need on_transfer_completed to
                    // free memory.
                    None => panic!("Memory leak!"),
                };
            }
        }
    }

    #[test]
    fn submit_transfer_no_callback_test() {
        let t = control_transfer(0);
        test_utils::fake_submit_transfer(t);
        let t = bulk_transfer(0, 0, 1);
        test_utils::fake_submit_transfer(t);
    }

    struct FakeTransferController {
        data: Mutex<u8>,
    }

    #[test]
    fn submit_transfer_with_callback() {
        let c = Arc::new(FakeTransferController {
            data: Mutex::new(0),
        });
        let c1 = Arc::downgrade(&c);
        let mut t = control_transfer(0);
        t.set_callback(move |_t| {
            let c = c1.upgrade().unwrap();
            *c.data.lock().unwrap() = 3;
        });
        test_utils::fake_submit_transfer(t);
        assert_eq!(*c.data.lock().unwrap(), 3);
    }
}
