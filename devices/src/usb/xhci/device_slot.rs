// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.


// See xhci spec page 55 for more details about device slot.
// Each usb device is represented by an entry in the Device Context Base Address
// Array, a register in the Doorbell Array register, and a device's Device
// Context.
pub struct DeviceSlot {
    // SlotId is the index used to identify a specific Device Slot in the Device
    // Context Base Address Array.
    SlotId: u8,
}
