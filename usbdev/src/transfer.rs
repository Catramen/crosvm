// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
// Generated with bindgen usbdevice_fs.h -no-prepend-enum-name -o bindings.rs.

use std::os::raw::c_void;
use error::*;
use bindings;
use types::UsbRequestSetup;

use std::os::raw::c_uchar;

/// Trait for usb transfer buffer.
/// Note: in the future, we can impl this for (GuestMemory, Offset, Length) and enable direct
/// access to guest memory.
pub trait UsbTransferBuffer: Send {
    fn as_ptr(&mut self) -> *mut u8;
    fn len(&self) -> i32;
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

    /// Set request setup for this control buffer.
    pub fn set_request_setup(&mut self, request_setup: &UsbRequestSetup) {
        self.setup_buffer = request_setup.clone();
    }
}

impl UsbTransferBuffer for ControlTransferBuffer {
    fn as_ptr(&mut self) -> *mut u8 {
        self as *mut ControlTransferBuffer as *mut u8
    }

    fn len(&self) -> i32 {
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
    fn with_size(buffer_size: usize) -> Self {
        BulkTransferBuffer {
            buffer: vec![0; buffer_size],
        }
    }

    /// Get mutable interal slice of this buffer.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.buffer
    }

    /// Get interal slice of this buffer.
    pub fn as_slice(&self) -> &[u8] {
        &self.buffer
    }
}

impl UsbTransferBuffer for BulkTransferBuffer {
    fn as_ptr(&mut self) -> *mut u8 {
        if self.buffer.len() == 0 {
            // Vec::as_mut_ptr() won't give 0x0 even if len() is 0.
            std::ptr::null_mut()
        } else {
            self.buffer.as_mut_ptr()
        }
    }

    fn len(&self) -> i32 {
        self.buffer.len() as i32
    }
}

type UsbTransferCompletionCallback<T> = Fn(UsbTransfer<T>) + Send + 'static;


/// TransferCanceller can cancel the transfer.
pub struct TransferCanceller {
}

impl TransferCanceller {
    /// Return false if fail to cancel.
    pub fn try_cancel(&self) -> bool {
        true
    }
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
        BulkTransferBuffer::with_size(size),
    )
}

/// Build a data transfer.
pub fn interrupt_transfer(
    endpoint: u8,
    timeout: u32,
    size: usize,
) -> UsbTransfer<BulkTransferBuffer> {
    UsbTransfer::<BulkTransferBuffer>::new(
        endpoint,
        LIBUSB_TRANSFER_TYPE_INTERRUPT as u8,
        timeout,
        BulkTransferBuffer::with_size(size),
    )
}

struct UsbTransferInner<T: UsbTransferBuffer> {
    urb: Arc<bindings::usbdevfs_urb>,
    callback: Option<Box<UsbTransferCompletionCallback<T>>>,
    buffer: T,
}

/// UsbTransfer owns a LibUsbTransfer, it's buffer and callback.
pub struct UsbTransfer<T: UsbTransferBuffer> {
    inner: Box<UsbTransferInner<T>>,
}

impl<T: UsbTransferBuffer> UsbTransfer<T> {
    fn new(endpoint: u8, type_: u8, timeout: u32, buffer: T) -> Self {
        let urb = usbdevfs_urb {
            type_: type_ as c_uchar,
            endpoint: endpoint as c_uchar,
            status: 0,
            flags: 0,
            buffer: std::ptr::null_mut(),
            buffer_length: 0,
            actual_length: 0,
            start_frame: 0,
            __bindgen_anon_1: usbdevfs_urb__bindgen_ty_1 {
                number_of_packets: 0
            },
            error_count: 0,
            signr: 0,
            usercontext: std::ptr::null_mut(),
            iso_frame_desc: __IncompleteArrayField::new()
        }
        let inner = UsbTransferInner {
            urb: Arc::new(urb),
            callback: None,
            buffer,
        };
        UsbTransfer { Box::new(inner) }
    }

    /// Get canceller of this transfer.
    //pub fn get_canceller(&self) -> TransferCanceller {
    //}

    /// Set callback function for transfer completion.
   // pub fn set_callback<C: 'static + Fn(UsbTransfer<T>) + Send>(&mut self, cb: C) {
   //     self.inner.callback = Some(Box::new(cb));
   // }

    /// Get a reference to the buffer.
    pub fn buffer(&self) -> &T {
        &self.buffer
    }

    /// Get a mutable reference to the buffer.
    pub fn buffer_mut(&mut self) -> &mut T {
        &mut self.buffer
    }

    /// Get actual length of data that was transferred.
    pub fn actual_length(&self) -> i32 {
        self.inner
    }

    /// Get the transfer status of this transfer.
    pub fn status(&self) -> TransferStatus {
        let transfer = self.inner.transfer.ptr;
        // Safe because inner.ptr is always allocated by libusb_alloc_transfer.
        unsafe { TransferStatus::from((*transfer).status) }
    }

    /// Invoke callback when transfer is completed.
    pub fn on_transfer_completed(self) {
        if let Some(cb) = transfer.inner.callback.take() {
            cb(transfer);
        }
    }

    /*
    fn into_raw(mut self) -> *mut libusb_transfer {
        let transfer: *mut libusb_transfer = self.inner.transfer.ptr;
        // Safe because transfer is allocated by libusb_alloc_transfer.
        unsafe {
            (*transfer).buffer = self.buffer_mut().as_ptr();
            (*transfer).length = self.buffer_mut().len();
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
    }*/
}

