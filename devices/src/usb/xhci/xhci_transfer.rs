// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.


type TransferTrb = AddressedTrb;
impl AddressedTrb {
    pub fn is_valid(&self, max_interrupters: u8) -> bool {
        self.trb.can_in_transfer_ring() &&
            (self.trb.interrupter_target() <= max_interrupters)
    }
}

pub struct XhciTransfer {
    transfer_trbs: Vec<TransferTrb>,
    usb_transfer: Option<UsbTransfer>,
    mem: GuestMemory,
}

impl XhciTransfer {
    pub fn new() -> Self {
        XhciTransfer {
        }
    }

    // Asynchronously submit the transfer to the backend. The callback will be
    // executed upon completion.
    pub fn submit(&self, callback: Box<Fn()>, backend: UsbBackend) {
        // Somehow really submit.
        backend.submit_transfer();
    }

    // Check each trb in the transfer descriptor for invalid or out of bounds
    // parameters. Returns true iff the transfer descriptor is valid.
    pub fn validate_trb(&self, max_interrupters: u32) -> Result<(), Vec<GuestAddress>> {
        let invalid_vec = Vec::new();
        for trb in self.transfer_trbs {
            if !trb.is_valid() {
                invalid_vec.push(trb.gpa());
            }
        }
        if invalid_vec.is_empty() {
            Ok(())
        } else {
            Err(invalid_vec)
        }
    }

    // Total bytes transferred in this transfer.
    pub fn bytes_transferred(&self) -> u32 {
        match self.usb_transfer {
            Some(t) => t.bytes_transferred(),
            None => 0,
        }
    }
}

