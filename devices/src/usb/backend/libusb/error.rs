// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std;
use std::fmt;

use usb::libusb::bindings::*;

pub enum Error {
    SUCCESS(i32),
    IO,
    INVALID_PARAM,
    ACCESS,
    NO_DEVICE,
    NOT_FOUND,
    BUSY,
    TIMEOUT,
    OVERFLOW,
    PIPE,
    INTERRUPTED,
    NO_MEM,
    NOT_SUPPORTED,
    OTHER,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::SUCCESS(_v) => write!(f, "Success (no error)"),
            &Error::IO => write!(f, "Input/output error"),
            &Error::INVALID_PARAM => write!(f, "Invalid parameter"),
            &Error::ACCESS => write!(f, "Access denied (insufficient permissions)"),
            &Error::NO_DEVICE => write!(f, "No such device (it may have been disconnected)"),
            &Error::NOT_FOUND => write!(f, "Entity not found"),
            &Error::BUSY => write!(f, "Resource busy"),
            &Error::TIMEOUT => write!(f, "Operation timed out"),
            &Error::OVERFLOW => write!(f, "Overflow"),
            &Error::PIPE => write!(f, "Pipe error"),
            &Error::INTERRUPTED => write!(f, "System call interrupted (perhaps due to signal)"),
            &Error::NO_MEM => write!(f, "Insufficient memory"),
            &Error::NOT_SUPPORTED => write!(f, "Operation not supported or unimplemented on this platform"),
            &Error::OTHER => write!(f, "Other error"),
        }
    }
}

impl Error {
    pub fn new(e: libusb_error) -> Error {
        match e {
            LIBUSB_ERROR_IO => Error::IO,
            LIBUSB_ERROR_INVALID_PARAM => Error::INVALID_PARAM,
            LIBUSB_ERROR_ACCESS => Error::ACCESS,
            LIBUSB_ERROR_NO_DEVICE => Error::NO_DEVICE,
            LIBUSB_ERROR_NOT_FOUND => Error::NOT_FOUND,
            LIBUSB_ERROR_BUSY => Error::BUSY,
            LIBUSB_ERROR_TIMEOUT => Error::TIMEOUT,
            LIBUSB_ERROR_OVERFLOW => Error::OVERFLOW,
            LIBUSB_ERROR_PIPE => Error::PIPE,
            LIBUSB_ERROR_INTERRUPTED => Error::INTERRUPTED,
            LIBUSB_ERROR_NO_MEM => Error::NO_MEM,
            LIBUSB_ERROR_NOT_SUPPORTED => Error::NOT_SUPPORTED,
            LIBUSB_ERROR_OTHER => Error::OTHER,
            // All possible erros are defined above, other values mean success,
            // see libusb_get_device_list for example.
            _ => Error::SUCCESS(e),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

macro_rules! call_libusb_fn {
    ($x:expr) => {
        match unsafe { Error::new($x as i32) } {
            Error::SUCCESS(e) => e,
            err => return Err(err),
        }
    }
}
