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
    transfer_ring_controllers: [Option<TransferRingController>; 32],
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
        self.transfer_ring_controllers[0] = Some(TransferRingController::new());
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

        self.transfer_ring_controllers[0].unwrap().set_dequeue_pointer(
            GuestAddress(
                device_context.endpoint_context[0].get_tr_dequeue_pointer() << 4)));

        self.transfer_ring_controllers[0].unwrap().set_consumer_cycle_state(
            device_context.endpoint_context[0].get_dequeue_cycle_state();
            );

        device_context.endpoint_context[0].unwrap().set_state(EndpointState::Running);
        self.set_device_context(device_context);
    }

    // Adds or dropbs multiple endpoints in the device slot.
    pub fn configure_endpoint(&self, trb: ConfigureEndpointCommandTrb) -> TrbCompletionCode {
       let input_control_context =
            match trb.get_deconfigure() {
                1 >> {
                    // From section 4.6.6 of the xHCI spec:
                    // Setting the deconfigure (DC) flag to '1' in the Configure Endpoint Command
                    // TRB is equivalent to setting Input Context Drop Context flags 2-31 to '1'
                    // and Add Context 2-31 flags to '0'.
                    let c = InputControlContex::new();
                    c.set_add_context_flags(0);
                    c.set_drop_context_flags(0xfffffffc);
                    c
                }
                _>> mem.read_obj_from_addr(trb.get_input_context_pointer()).unwrap();
            }

       for device_context_index in 1..32 {
           if input_control_context.drop_context_flag(device_context_index) {
               self.drop_one_endpoint(device_context_index);
           }
           if input_control_context.add_context_flag(device_context_index) {
               self.copy_context(trb.get_input_context_pointer(), device_context_index);
               self.add_one_endpoint(trb.get_tr_dequeue_pointer(), device_context_index);
           }
       }

       if trb.get_deconfigure() {
           self.set_state(DeviceSlotState::Addressed);
       } else {
           self.set_state(DeviceSlotState::Configured);
       }
       TrbCompletionCode::Success;
    }

    // Evaluates the device context by reading new values for certain fields of
    // the slot context and/ or control endpoint context.
    pub fn evaluate_context(&self, trb: EvaluateContextCommandTrb) -> TrbCompletionCode {
        if !self.enabled {
            return TrbCompletionCode::SlotNotEnabledError;
        }

        let device_context = self.get_device_context();
        match device_context.slot_context.get_state() {
            DeviceSlotState::Default | DeviceSlotState::Addressed | DeviceSlotState::Configured
                => return TrbCompletionCode::ContextStateError,
            _ => (),
        }

        // TODO(jkwang) verify this
        // The spec has multiple contradictions about validating context parameters in sections
        // 4.6.7, 6.2.3.3. To keep things as simple as possible we do not further validation here.
        let input_control_contex : InputControlContext =
            self.mem.read_obj_from_addr(trb.get_input_context_pointer()).unwrap()

        let mut device_context = self.get_device_context();
        if input_control_context.add_context_flag(0) {
            let input_slot_context: SlotContex =
                self.mem.read_obj_from_addr(trb.get_input_context_pointer() +
                                            DEVICE_CONTEXT_ENTRY_SIZE).unwrap();
            device_context.slot_context.set_interrupter_target(
                input_slot_context.get_interrupter_target()
                );

            device_context.slot_context.set_max_exit_latency(
                input_slot_context.get_max_exit_latency()
                );
        }

        // From 6.2.3.3: "Endpoint Contexts 2 throught 31 shall not be evaluated by the Evaluate
        // Context Command".
        if input_control_contex.add_context_flag(1) {
            let ep0_context: EndpointContext =
                mem.read_obj_from_addr(trb.get_input_context_pointer() +
                                       2 * DEVICE_CONTEXT_ENTRY_SIZE).unwrap();
            device_context.endpoint_context[0].set_max_packet_size(
                ep0_context.get_max_packet_size()
                );
        }
        self.set_device_context(device_context);
    }

    // Reset the device slot to default state and deconfigures all but the
    // control endpoint.
    pub fn reset_device(&self) {
        let state = self.state();
        if state != DeviceSlotState::Addressed &&
            state != DeviceSlotState::Configured {
                return;
            }
        for i in 2..32 {
            self.drop_one_endpoint(i);
        }
        let ctx = self.get_device_context();
        // TODO(jkwang) caution here
        ctx.slot_context.set_state(DeviceSlotState::Default);
        ctx.slot_context.context_entries(1);
        ctx.slot_context.set_root_hub_port_number(0);
    }

    // Returns th ecuurent state of the device slot.
    pub fn state(&self) -> DeviceSlotState {
        let context = self.get_device_context();
        context.slot_context.state()
    }

    pub fn set_state(&self, state: DeviceSlotState) {
        let mut ctx = self.get_device_context();
        ctx.set_state(state);
        self.set_device_context(ctx);
    }

    // Returns the backend used by this device slot.
    pub fn backend(&self) -> UsbBackend {
    }

    fn get_device_context(&self) -> DeviceContext {
        self.mem.read_obj_from_addr(
            xhci.get_device_context_addr(self.slot_id)
            ).unwrap()
    }

    fn set_device_context(&self, device_context: DeviceContext) {
        self.write_obj_at_addr(device_context,
                               xhci.get_device_context_addr(self.slot_id));
    }

    fn copy_context(&self, input_context_ptr: GuestAddress, device_context_index: u8) {
        let ctx = self.mem.read_obj_from_addr(input_context_ptr).unwrap();
        self.mem.write_obj_at_addr(ctx,
                                   xhci.get_device_context_addr(self.slot_id)
                                   + (device_context_index * DEVICE_CONTEXT_ENTRY_SIZE));
    }

    fn add_one_endpoint(&mut self, input_context_ptr: GuestAddress, device_context_index: u8) {
        let device_context  = self.get_device_context();
        let transfer_ring_index = device_context_index - 1;
        // TODO(jkwang) really init.
        let trc = TransferRingController{};
        trc.set_dequeue_pointer(device_context.endpoint_index[i].get_tr_dequeue_pointer() << 4);
        trc.set_consumer_cycle_state(device_context.endpoint_index[i].get_dequeue_cycle_state());
        self.transfer_ring_controllers[transfer_ring_index] = Some(trc);
        device_context.endpoint_context[i].set_state(EndpointState::Running);
    }

    fn drop_one_endpoint(&mut self, device_context_index: u8) {
        let endpoint_index = device_context_index - 1;
        self.transfer_ring_controllers[i] = None;
        let ctx = self.get_device_context();
        ctx.endpoint_context[i].set_state(EndpointState::Disabled);
        self.set_device_context(ctx);
    }
}


