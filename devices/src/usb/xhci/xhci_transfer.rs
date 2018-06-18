// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.


pub struct XhciTransfer {
    transfer_trbs: Vec<TransferTrb>,
    guest_memory: GuestMemory,
}

impl XhciTransfer {
    pub fn new() -> Self {
        XhciTransfer {
        }
    }

    // Asynchronously submit the transfer to the backend
    pub fn submit(callback: Box<Fn()>) {
    }

    pub fn is_valid() -> bool {

    }
}

