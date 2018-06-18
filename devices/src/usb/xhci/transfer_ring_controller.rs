// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

type Result<T> = std::result::Result<T, Error>;

pub struct TransferRingController {
    // Slot IDs are one-based.
    slot_id: u8,

    // Endpoint ID is equivalent to device context index: 1 is EP0 followed by EP1
    // out, EP1 in, EP2 out etc. The endpoint nuber is equal to this value divided
    // by two.
    endpoint_id: u8,
    device_slot: DeviceSlot,
}

type TransferTrb = AddressedTrb;
impl AddressedTrb {
    pub fn is_valid(&self, max_interrupters: u8) -> bool {
        self.trb.can_in_transfer_ring() &&
            (self.trb.interrupter_target() <= max_interrupters)
    }
}

// Public
impl TransferRingController {
    pub fn new(slot_id: u8, endpoint_id: u8, device_slot: DeviceSlot) {
        TransferRingController {
            slot_id: slot_id,
            endpoint_id: endpoint_id,
            device_slot: device_slot,
        }
    }

    pub fn handle_transfer_descriptor(&self, descriptor: &[AddressedTrb],
                                      callback: Callback) {
    }
}



#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
    }
}
