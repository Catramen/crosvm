// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use data_model::DataInit;
use bindings;

/// Speed of usb device. See usb spec for more details.
#[derive(Debug)]
pub enum Speed {
    // The OS doesn't report or know the device speed.
    Unknown,
    // The device is operating at low speed (1.5MBit/s).
    Low,
    // The device is operating at full speed (12MBit/s).
    Full,
    // The device is operating at high speed (480MBit/s).
    High,
    // The device is operating at super speed (5000MBit/s).
    Super,
}

impl From<bindings::libusb_speed> for Speed {
    fn from(speed: bindings::libusb_speed) -> Speed {
        match speed {
            bindings::LIBUSB_SPEED_LOW => Speed::Low,
            bindings::LIBUSB_SPEED_FULL => Speed::Full,
            bindings::LIBUSB_SPEED_HIGH => Speed::High,
            bindings::LIBUSB_SPEED_SUPER => Speed::Super,
            _ => Speed::Unknown,
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct UsbRequestSetup {
    // USB Device Request. USB spec. rev. 2.0 9.3
    pub request_type: u8,    // bmRequestType
    pub request: u8,       // bRequest
    pub value: u16,        // wValue
    pub index: u16,        // wIndex
    pub length: u16,       // wLength
}

unsafe impl DataInit for UsbRequestSetup {}

/// Usb transfer types. See spec for more details.
pub enum TransferType {
    In,
    Out,
    Setup,
}

#[cfg(test)]
mod tests {
    use std::mem::size_of;
    use super::*;

    #[test]
    fn check_request_setup_size() {
        assert_eq!(size_of::<UsbRequestSetup>(), 8);
    }
}
