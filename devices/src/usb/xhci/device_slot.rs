// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// See xhci spec page 55 for more details about device slot.
// Each usb device is represented by an entry in the Device Context Base Address
// Array, a register in the Doorbell Array register, and a device's Device
// Context.
pub struct DeviceSlot {
    mem: GuestMemory,
    // slot_id is the index used to identify a specific Device Slot in the Device
    // Context Base Address Array.
    slot_id: u8,

    enabled: bool,
    backend: UsbBackend,
    transfer_ring_controllers: [Option<TransferRing>; 32],
    xhci: XHCI,
}

impl DeviceSlot {

    // The arguemtns are identical to the fields in each doorbell register. The
    // target value:
    // 1: Reserved
    // 2: Control endpoint
    // 3: Endpoint 1 out
    // 4: Endpoint 1 in
    // 5: Endpoint 2 out
    // ...
    // 32: Endpoint 15 in
    //
    // The stream ID must be zero for endpoints that do not have streams
    // configured.
    pub fn ring_doorbell(&self, target: u8, stream_id: u16) {
        if target < 1 || target > 31 {
            panic!("Invalid target written to doorbell register");
        }

        let i = target - 1;
        let transfer_ring_controller = match self.transfer_ring_controllers[i].as_ref() {
            Some(tr) => tr,
            None => panic!("Endpoint is not set");
        }
        let context = self.get_device_context();
        if context.state() == EndpointState::Running {
            transfer_ring_controller.start();
        }
    }

    // Enable the slot, return if it's successful.
    pub fn enable(&self) -> bool {
        if self.enabled {
            return false;
        }

        // TODO(jkwang) fix this.
        self.transfer_ring_controllers[0] = TransferRingController::new();
        self.enable = true
    }

    // Disable the device slot.
    pub fn disable(&self) {
        if self.enabled {
            for trc in self.transfer_ring_controllers {
                // TODO(jkwang)
                trc.stop();
            }
        } else {
            // TODO(jkwang) fix this
            panic!("not enabled error");
        }
    }

    // Assigns the device address and initializes slot and endpoint 0 context.
    pub fn set_address(&self, trb: AddressDeviceCommandTrb) -> TrbCompletionCode {
        if !self.enabled {
            return TrbCompletionCode::SlotNotEnabledError;
        }

        if ( self.state() != DeviceSlotState::DisabledOrEnabled ) &&
            ( self.state() != DeviceSlotState::Default ||  trb.get_block_set_address_request()) {
                return TrbCompletionCode::ContextStateError;
            }

        // Copy all fields of the slot context and endpoint 0 context from the input context
        // to the output context.
        let input_context_addr = GuestAddress(trb.get_input_context_pointer());
        self.copy_context(input_context_pointer, 0);
        self.copy_context(input_context_pointer, 1);
        let mut device_context = self.get_device_context();
        // TODO refactor this
        self.backend = get_backend_from_some_where();

        // Assign slot ID as device address if block_set_address_request is not set.
        if !trb.get_block_set_address_request() {
            if there_is_backend {
                backend.set_address(self.slot_id);
            } else {
                return TrbCompletionCode::TransactionError;
            }
            device_context.slot_context.set_usb_device_address(self.slot_id);
            device_context.slot_context.set_state(DeviceSlotState::Addressed);
        } else {
            device_context.slot_context.set_state(DeviceSlotState::Default);
        }

        self.set_device_context(device_context);

    }

    // Adds or dropbs multiple endpoints in the device slot.
    pub fn configure_endpoint(&self, trb: ConfigureEndpointCommandTrb) {
    }

    // Evaluates the device context by reading new values for certain fields of
    // the slot context and/ or control endpoint context.
    pub fn evaluate_context(&self, trb: EvaluateContextCommandTrb) {
    }

    // Reset the device slot to default state and deconfigures all but the
    // control endpoint.
    pub fn reset_device(&self) {
    }

    // Returns th ecuurent state of the device slot.
    pub fn state(&self) -> DeviceSlotState {
        let context = self.get_device_context();
        context.slot_context.state()
    }

    pub fn set_state(&self, state: DeviceSlotState) {
    }

    // Returns the backend used by this device slot.
    pub fn backend(&self) -> UsbBackend {
    }

    fn get_device_context(&self) -> DeviceContext {
        // TODO address
        self.mem.read_obj_from_addr().unwrap()
    }

    fn set_device_context(&self, device_context: DeviceContext) {
        // Reall set device context.
    }

    fn copy_context(&self, input_context_pointer: GuestAddress, device_context_index: u8) {
    }
}

