// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::Arc;
use std::os::raw::c_void;

use error::Error;
use types::UsbRequestSetup;
use bindings::{
    libusb_transfer,
    libusb_alloc_transfer,
    libusb_free_transfer,
    libusb_device_handle,
    libusb_submit_transfer,
    libusb_transfer_status,
    LIBUSB_TRANSFER_TYPE_CONTROL,
    LIBUSB_TRANSFER_TYPE_BULK,
    LIBUSB_TRANSFER_COMPLETED,
    LIBUSB_TRANSFER_ERROR,
    LIBUSB_TRANSFER_TIMED_OUT,
    LIBUSB_TRANSFER_CANCELLED,
    LIBUSB_TRANSFER_STALL,
    LIBUSB_TRANSFER_NO_DEVICE,
    LIBUSB_TRANSFER_OVERFLOW,
};

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

/// LibUsbTransfer owns raw transfer pointer, it will call free transfer to free libusb_transfer
/// memory.
pub struct LibUsbTransfer {
    transfer: *mut libusb_transfer,
}

// TODO(jkwang) Add more utility functions for LibUsbTransfer.
impl LibUsbTransfer {
    fn new() -> LibUsbTransfer {
        // Safe because alloc is safe.
        let transfer: *mut libusb_transfer = unsafe { libusb_alloc_transfer(0) };
        // Just panic on OOM.
        assert!(!transfer.is_null());
        LibUsbTransfer {
            transfer,
        }
    }

    pub fn status(&self) -> TransferStatus {
        TransferStatus::from(self.raw_ref().status)
    }

    fn raw_ref(&self) -> &libusb_transfer {
        // Safe because 'self.transfer' is valid.
        unsafe {
            &*(self.transfer)
        }
    }

    fn raw_ref_mut(&mut self) -> &mut libusb_transfer {
        // Safe because 'self.transfer' is valid.
        unsafe {
            &mut *(self.transfer)
        }
    }

    fn raw(&mut self) -> *mut libusb_transfer {
        self.transfer
    }
}

impl Drop for LibUsbTransfer {
    fn drop(&mut self) {
        // Safe because 'self.transfer' is valid.
        unsafe {
            libusb_free_transfer(self.transfer);
        }
    }
}

/// Trait for usb transfer buffer.
pub trait UsbTransferBuffer: Default {
    fn as_raw_ptr(&mut self) -> *mut u8;
    fn length(&self) -> i32;
}

/// Default buffer size for control data transfer.
const CONTROL_DATA_BUFFER_SIZE: usize = 1024;

/// Buffer type for control transfer. The first 8-bytes is a UsbRequestSetup struct.
#[repr(C, packed)]
pub struct ControlTransferBuffer {
    setup_buffer: UsbRequestSetup,
    data_buffer: [u8; CONTROL_DATA_BUFFER_SIZE],
}

impl Default for ControlTransferBuffer {
    fn default() -> ControlTransferBuffer {
        ControlTransferBuffer{
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
}

impl ControlTransferBuffer {
    /// Returns a mutable reference of setup_buffer.
    pub fn request_setup(&mut self) -> &mut UsbRequestSetup {
        &mut (self.setup_buffer)
    }
}

impl UsbTransferBuffer for ControlTransferBuffer {
    fn as_raw_ptr(&mut self) -> *mut u8 {
        self as *mut ControlTransferBuffer as *mut u8
    }

    fn length(&self) -> i32 {
        self.setup_buffer.length as i32
    }
}

// TODO(jkwang) investigate and optimize bulk buffer size to save memory allocation.
/// Default buffer size for control data transfer.
const CONTROL_BUFFER_SIZE: usize = 1024;

/// Buffer type for Bulk transfer.
pub struct BulkTransferBuffer {
    buffer: Vec<u8>,
}

impl Default for BulkTransferBuffer {
    fn default() -> Self {
        BulkTransferBuffer {
            buffer: vec![0; CONTROL_BUFFER_SIZE],
        }
    }
}

impl BulkTransferBuffer {
    fn vec(&self) -> &Vec<u8> {
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

type UsbTransferCompletionCallback<T> = Fn(UsbTransfer<T>);

struct UsbTransferImpl<T: UsbTransferBuffer> {
    transfer: LibUsbTransfer,
    callback: Option<Arc<UsbTransferCompletionCallback<T>>>,
    buffer: T,
}

/// UsbTransfer owns a libust_transfer, it's buffer and callback.
pub struct UsbTransfer<T: UsbTransferBuffer> (Box<UsbTransferImpl<T>>);

/// Build a control transfer.
pub fn control_transfer(timeout: u32) -> UsbTransfer<ControlTransferBuffer> {
    UsbTransfer::<ControlTransferBuffer>::new(0,
                                              LIBUSB_TRANSFER_TYPE_CONTROL as u8,
                                              timeout)
}

/// Build a data transfer.
pub fn bulk_transfer(endpoint:u8, timeout: u32) -> UsbTransfer<BulkTransferBuffer> {
    UsbTransfer::<BulkTransferBuffer>::new(endpoint,
                                           LIBUSB_TRANSFER_TYPE_BULK as u8,
                                           timeout)
}

impl<T: UsbTransferBuffer> UsbTransfer<T> {
    fn new(endpoint: u8, type_: u8, timeout: u32) -> Self {
        let mut transfer = Box::new(
            UsbTransferImpl::<T> {
                transfer: LibUsbTransfer::new(),
                callback: None,
                buffer: Default::default(),
            }
            );
        {
            let raw = transfer.transfer.raw_ref_mut();
            raw.endpoint = endpoint;
            raw.type_ = type_;
            raw.timeout = timeout;
            raw.callback = Some(transfer_completion_callback::<T>);
        }
        UsbTransfer(transfer)

    }

    /// Set callback function for transfer completion.
    pub fn set_callback<C: 'static + Fn(UsbTransfer<T>)>(&mut self, cb: C) {
        self.0.callback = Some(Arc::new(cb));
    }

    /// Get a reference to the LibUsbTransfer.
    pub fn transfer(&mut self) -> &mut LibUsbTransfer {
        &mut self.0.transfer
    }

    /// Get a reference to the buffer.
    pub fn buffer(&mut self) -> &mut T {
        &mut self.0.buffer
    }

    /// Submit this transfer to device handle. 'self' is consumed. On success, the memory will be
    /// 'leaked' (and store in user_data) and send to libusb, when the async operation is done,
    /// on_transfer_completed will recreate 'self' and deliver it to callback/free 'self'. On
    /// faliure, 'self' is returned with an error.
    pub unsafe fn submit(self, handle: *mut libusb_device_handle) ->
        Result<(), (Error, UsbTransfer<T>) > {
        let transfer = self.into_raw();
        (*transfer).dev_handle = handle;
        match Error::from(libusb_submit_transfer(transfer)) {
            Error::Success(_e) => Ok(()),
            err => Err((err, UsbTransfer::<T>::from_raw(transfer)))
        }
    }

    /// Invoke callback when transfer is completed.
    unsafe fn on_transfer_completed(transfer: *mut libusb_transfer) {
        let transfer = UsbTransfer::<T>::from_raw(transfer);
        let cb = match transfer.0.callback {
            Some(ref cb) => cb.clone(),
            None => return,
        };
        cb(transfer);
    }

    fn into_raw(mut self) -> *mut libusb_transfer {
        let transfer: *mut libusb_transfer = self.transfer().raw();
        // Safe because transfer is valid.
        unsafe {
            (*transfer).buffer = self.buffer().as_raw_ptr();
            (*transfer).length = self.buffer().length();
            (*transfer).user_data = Box::into_raw(self.0) as *mut c_void;
        }
        transfer
    }

    unsafe fn from_raw(transfer: *mut libusb_transfer) -> Self {
        UsbTransfer(Box::<UsbTransferImpl<T>>::from_raw(
                (*transfer).user_data as *mut UsbTransferImpl<T>))
    }
}

/// Unsafe code for transfer completion handling.
pub unsafe extern "C" fn transfer_completion_callback<T: UsbTransferBuffer>(
    transfer: *mut libusb_transfer) {
    UsbTransfer::<T>::on_transfer_completed(transfer);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;
    use std::sync::{Arc, Mutex};

    #[test]
    fn check_control_buffer_size() {
        assert_eq!(size_of::<ControlTransferBuffer>(),
                   size_of::<UsbRequestSetup>() + CONTROL_DATA_BUFFER_SIZE);
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
        let t = bulk_transfer(0, 0);
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
