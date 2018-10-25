// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::interrupter::Interrupter;
use super::mmio_register::Register;
use super::transfer_ring_controller::TransferRingController;
use super::usb_hub::UsbHub;
use super::usb_hub::UsbPort;
use super::xhci_abi::{
    AddressDeviceCommandTrb, AddressedTrb, ConfigureEndpointCommandTrb, DeviceContext,
    DeviceSlotState, EndpointContext, EndpointState, EvaluateContextCommandTrb,
    InputControlContext, SlotContext, TrbCompletionCode, DEVICE_CONTEXT_ENTRY_SIZE,
};
use super::xhci_regs::{valid_slot_id, MAX_SLOTS};
use std::mem::size_of;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::sync::{Arc, Mutex, MutexGuard};
use sys_util::{EventFd, GuestAddress, GuestMemory};
use usb::auto_callback::AutoCallback;
use usb::event_loop::{EventHandler, EventLoop};

/// See spec 4.5.1 for dci.
/// index 0: Control endpoint. Device Context Index: 1.
/// index 1: Endpoint 1 out. Device Context Index: 2
/// index 2: Endpoint 1 in. Device Context Index: 3.
/// index 3: Endpoint 2 out. Device Context Index: 4
/// ...
/// index 30: Endpoint 15 in. Device Context Index: 31
pub const TRANSFER_RING_CONTROLLERS_INDEX_END: usize = 31;
pub const DCI_INDEX_END: usize = TRANSFER_RING_CONTROLLERS_INDEX_END + 1;
pub const FIRST_TRANSFER_ENDPOINT_DCI: usize = 2;

fn valid_endpoint_id(endpoint_id: u8) -> bool {
    endpoint_id < DCI_INDEX_END as u8 && endpoint_id > 0
}

#[derive(Clone)]
pub struct DeviceSlots {
    hub: Arc<UsbHub>,
    slots: Vec<Arc<Mutex<DeviceSlot>>>,
}

impl DeviceSlots {
    pub fn new(
        dcbaap: Register<u64>,
        hub: Arc<UsbHub>,
        interrupter: Arc<Mutex<Interrupter>>,
        event_loop: EventLoop,
        mem: GuestMemory,
    ) -> DeviceSlots {
        let mut vec = Vec::new();
        for i in 0..MAX_SLOTS {
            let slot_id = i + 1;
            vec.push(Arc::new(Mutex::new(DeviceSlot::new(
                slot_id as u8,
                dcbaap.clone(),
                hub.clone(),
                interrupter.clone(),
                event_loop.clone(),
                mem.clone(),
            ))));
        }
        DeviceSlots { hub, slots: vec }
    }

    /// Note that slot id starts from 0. Slot index start from 1.
    pub fn slot(&self, slot_id: u8) -> Option<MutexGuard<DeviceSlot>> {
        if !valid_slot_id(slot_id) {
            error!(
                "trying to index a wrong slot id {}, max slot = {}",
                slot_id, MAX_SLOTS
            );
            None
        } else {
            Some(self.slots[slot_id as usize - 1].lock().unwrap())
        }
    }

    pub fn stop_all_and_reset<C: Fn() + 'static + Send>(&self, callback: C) {
        debug!("stoping all device slots and reset host hub");
        let slots = self.slots.clone();
        let hub = self.hub.clone();
        let auto_callback = AutoCallback::new(move || {
            debug!("executing stop device slot callback");
            for slot in &slots {
                slot.lock().unwrap().reset();
                hub.reset();
            }
            callback();
        });
        self.stop_all(auto_callback);
    }

    pub fn stop_all(&self, auto_callback: AutoCallback) {
        for slot in &self.slots {
            slot.lock().unwrap().stop_all_trc(auto_callback.clone());
        }
    }

    pub fn disable_slot<C: Fn(TrbCompletionCode) + 'static + Send>(&self, slot_id: u8, cb: C) {
        debug!("device slot {} is disabling", slot_id);
        DeviceSlot::disable(&self.slots[slot_id as usize - 1], cb);
    }

    pub fn reset_slot<C: Fn(TrbCompletionCode) + 'static + Send>(&self, slot_id: u8, cb: C) {
        debug!("device slot {} is reseting", slot_id);
        DeviceSlot::reset_slot(&self.slots[slot_id as usize - 1], cb);
    }
}

pub struct DeviceSlot {
    slot_id: u8,
    port_id: u8, // Valid port id starts from 1, to MAX_PORTS.
    dcbaap: Register<u64>,
    hub: Arc<UsbHub>,
    interrupter: Arc<Mutex<Interrupter>>,
    event_loop: EventLoop,
    mem: GuestMemory,
    enabled: bool,
    transfer_ring_controllers: Vec<Option<Arc<TransferRingController>>>,
}

impl DeviceSlot {
    pub fn new(
        slot_id: u8,
        dcbaap: Register<u64>,
        hub: Arc<UsbHub>,
        interrupter: Arc<Mutex<Interrupter>>,
        event_loop: EventLoop,
        mem: GuestMemory,
    ) -> Self {
        let mut transfer_ring_controllers = Vec::new();
        for _i in 0..TRANSFER_RING_CONTROLLERS_INDEX_END {
            transfer_ring_controllers.push(None);
        }
        DeviceSlot {
            slot_id,
            port_id: 0,
            dcbaap,
            hub,
            interrupter,
            event_loop,
            mem,
            enabled: false,
            transfer_ring_controllers,
        }
    }

    /// The arguemtns are identical to the fields in each doorbell register. The
    /// target value:
    /// 1: Reserved
    /// 2: Control endpoint
    /// 3: Endpoint 1 out
    /// 4: Endpoint 1 in
    /// 5: Endpoint 2 out
    /// ...
    /// 32: Endpoint 15 in
    ///
    /// The stream ID must be zero for endpoints that do not have streams
    /// configured.
    pub fn ring_doorbell(&self, target: usize, _stream_id: u16) -> bool {
        if !valid_endpoint_id(target as u8) {
            error!(
                "device slot {}: Invalid target written to doorbell register. target: {}",
                self.slot_id, target
            );
            return false;
        }
        debug!(
            "device slot {}: ding-dong. who is that? target = {}",
            self.slot_id, target
        );
        // See DCI in spec.
        let endpoint_index = target - 1;
        let transfer_ring_controller = match self.transfer_ring_controllers[endpoint_index].as_ref()
        {
            Some(tr) => tr,
            None => {
                error!("Device endpoint is not inited");
                return false;
            }
        };
        let context = self.get_device_context();
        if context.endpoint_context[endpoint_index].get_endpoint_state()
            == EndpointState::Running as u8
        {
            debug!("endpoint is started, start transfer ring");
            transfer_ring_controller.start();
        } else {
            error!("door bell rung when endpoint is not started");
        }
        return true;
    }

    /// Enable the slot, return if true it's successful.
    pub fn enable(&mut self) -> bool {
        if self.enabled {
            error!("device slot is already enabled");
            return false;
        }
        debug!("device slot {} enabled", self.slot_id);
        self.enabled = true;
        return true;
    }

    pub fn disable<C: Fn(TrbCompletionCode) + 'static + Send>(
        slot: &Arc<Mutex<DeviceSlot>>,
        callback: C,
    ) {
        let s = slot.lock().unwrap();
        debug!("device slot {} is being disabled", s.slot_id);
        if s.enabled {
            let slot_weak = Arc::downgrade(slot);
            let auto_callback = AutoCallback::new(move || {
                let slot_arc = slot_weak.upgrade().unwrap();
                let mut slot = slot_arc.lock().unwrap();
                let mut device_context = slot.get_device_context();
                device_context
                    .slot_context
                    .set_state(DeviceSlotState::DisabledOrEnabled);
                slot.set_device_context(device_context);
                slot.reset();
                debug!(
                    "device slot {}: all trc disabled, sending trb",
                    slot.slot_id
                );
                callback(TrbCompletionCode::Success);
            });
            s.stop_all_trc(auto_callback);
        } else {
            callback(TrbCompletionCode::SlotNotEnabledError);
        }
    }

    // Assigns the device address and initializes slot and endpoint 0 context.
    pub fn set_address(&mut self, trb: &AddressDeviceCommandTrb) -> TrbCompletionCode {
        if !self.enabled {
            error!(
                "trying to set address to a disabled device slot {}",
                self.slot_id
            );
            return TrbCompletionCode::SlotNotEnabledError;
        }
        let mut device_context = self.get_device_context();
        if (device_context.slot_context.state().unwrap() != DeviceSlotState::DisabledOrEnabled)
            && (device_context.slot_context.state().unwrap() != DeviceSlotState::Default
                || trb.get_block_set_address_request() > 0)
        {
            error!("unexpected slot state {}", self.slot_id);
            return TrbCompletionCode::ContextStateError;
        }

        // Copy all fields of the slot context and endpoint 0 context from the input context
        // to the output context.
        let input_context_ptr = GuestAddress(trb.get_input_context_pointer());
        // Copy slot context.
        self.copy_context(input_context_ptr, 0);
        // Copy control endpoint context.
        self.copy_context(input_context_ptr, 1);

        // Read back device context.
        let mut device_context = self.get_device_context();
        self.port_id = device_context.slot_context.get_root_hub_port_number();
        debug!(
            "port id {} is assigned to slot id {}",
            self.port_id, self.slot_id
        );

        // Initialize the control endpoint. Endpoint id = 1.
        self.transfer_ring_controllers[0] = Some(TransferRingController::new(
            self.mem.clone(),
            self.hub.get_port(self.port_id).unwrap(),
            self.event_loop.clone(),
            self.interrupter.clone(),
            self.slot_id,
            1,
        ));

        // Assign slot ID as device address if block_set_address_request is not set.
        if trb.get_block_set_address_request() > 0 {
            device_context
                .slot_context
                .set_state(DeviceSlotState::Default);
        } else {
            let port = self.hub.get_port(self.port_id).unwrap();
            let mut backend = port.get_backend_device();
            if backend.is_some() {
                backend.as_mut().unwrap().set_address(self.slot_id as u32);
            } else {
                return TrbCompletionCode::TransactionError;
            }
            device_context
                .slot_context
                .set_usb_device_address(self.slot_id);
            device_context
                .slot_context
                .set_state(DeviceSlotState::Addressed);
        }

        self.transfer_ring_controllers[0]
            .as_ref()
            .unwrap()
            .set_dequeue_pointer(GuestAddress(
                device_context.endpoint_context[0].get_tr_dequeue_pointer() << 4,
            ));

        self.transfer_ring_controllers[0]
            .as_ref()
            .unwrap()
            .set_consumer_cycle_state(
                device_context.endpoint_context[0].get_dequeue_cycle_state() > 0,
            );

        debug!("Setting endpoint 0 to running");
        device_context.endpoint_context[0].set_state(EndpointState::Running);
        self.set_device_context(device_context);
        TrbCompletionCode::Success
    }

    // Adds or dropbs multiple endpoints in the device slot.
    pub fn configure_endpoint(&mut self, trb: &ConfigureEndpointCommandTrb) -> TrbCompletionCode {
        debug!("configuring endpoint");
        let input_control_context: InputControlContext = match trb.get_deconfigure() > 0 {
            true => {
                // From section 4.6.6 of the xHCI spec:
                // Setting the deconfigure (DC) flag to '1' in the Configure Endpoint Command
                // TRB is equivalent to setting Input Context Drop Context flags 2-31 to '1'
                // and Add Context 2-31 flags to '0'.
                let mut c = InputControlContext::new();
                c.set_add_context_flags(0);
                c.set_drop_context_flags(0xfffffffc);
                c
            }
            _ => self
                .mem
                .read_obj_from_addr(GuestAddress(trb.get_input_context_pointer()))
                .unwrap(),
        };

        for device_context_index in 1..32 {
            if input_control_context.drop_context_flag(device_context_index) {
                self.drop_one_endpoint(device_context_index);
            }
            if input_control_context.add_context_flag(device_context_index) {
                self.copy_context(
                    GuestAddress(trb.get_input_context_pointer()),
                    device_context_index,
                );
                self.add_one_endpoint(device_context_index);
            }
        }

        if trb.get_deconfigure() > 0 {
            self.set_state(DeviceSlotState::Addressed);
        } else {
            self.set_state(DeviceSlotState::Configured);
        }
        TrbCompletionCode::Success
    }

    // Evaluates the device context by reading new values for certain fields of
    // the slot context and/ or control endpoint context.
    pub fn evaluate_context(&self, trb: &EvaluateContextCommandTrb) -> TrbCompletionCode {
        if !self.enabled {
            return TrbCompletionCode::SlotNotEnabledError;
        }

        let device_context = self.get_device_context();
        let state = device_context.slot_context.state().unwrap();
        if state == DeviceSlotState::Default
            || state == DeviceSlotState::Addressed
            || state == DeviceSlotState::Configured
        {
            error!(
                "wrong context state on evaluate context. state = {:?}",
                state
            );
            return TrbCompletionCode::ContextStateError;
        }

        // TODO(jkwang) verify this
        // The spec has multiple contradictions about validating context parameters in sections
        // 4.6.7, 6.2.3.3. To keep things as simple as possible we do not further validation here.
        let input_control_context: InputControlContext = self
            .mem
            .read_obj_from_addr(GuestAddress(trb.get_input_context_pointer()))
            .unwrap();

        let mut device_context = self.get_device_context();
        if input_control_context.add_context_flag(0) {
            let input_slot_context: SlotContext = self
                .mem
                .read_obj_from_addr(GuestAddress(
                    trb.get_input_context_pointer() + DEVICE_CONTEXT_ENTRY_SIZE as u64,
                ))
                .unwrap();
            device_context
                .slot_context
                .set_interrupter_target(input_slot_context.get_interrupter_target());

            device_context
                .slot_context
                .set_max_exit_latency(input_slot_context.get_max_exit_latency());
        }

        // From 6.2.3.3: "Endpoint Contexts 2 throught 31 shall not be evaluated by the Evaluate
        // Context Command".
        if input_control_context.add_context_flag(1) {
            let ep0_context: EndpointContext = self
                .mem
                .read_obj_from_addr(GuestAddress(
                    trb.get_input_context_pointer() + 2 * DEVICE_CONTEXT_ENTRY_SIZE as u64,
                ))
                .unwrap();
            device_context.endpoint_context[0]
                .set_max_packet_size(ep0_context.get_max_packet_size());
        }
        self.set_device_context(device_context);
        TrbCompletionCode::Success
    }

    // Reset the device slot to default state and deconfigures all but the
    // control endpoint.
    pub fn reset_slot<C: Fn(TrbCompletionCode) + 'static + Send>(
        slot: &Arc<Mutex<DeviceSlot>>,
        callback: C,
    ) {
        let s = slot.lock().unwrap();
        let state = s.state();
        if state != DeviceSlotState::Addressed && state != DeviceSlotState::Configured {
            error!("reset slot failed due to context state error {:?}", state);
            callback(TrbCompletionCode::ContextStateError);
            return;
        }

        let weak_s = Arc::downgrade(&slot);
        let auto_callback = AutoCallback::new(move || {
            let arc_s = weak_s.upgrade().unwrap();
            let mut s = arc_s.lock().unwrap();
            for i in 2..32 {
                s.drop_one_endpoint(i);
            }
            let mut ctx = s.get_device_context();
            ctx.slot_context.set_state(DeviceSlotState::Default);
            ctx.slot_context.set_context_entries(1);
            ctx.slot_context.set_root_hub_port_number(0);
            s.set_device_context(ctx);
            callback(TrbCompletionCode::Success);
        });
        s.stop_all_trc(auto_callback);
    }

    pub fn stop_all_trc(&self, auto_callback: AutoCallback) {
        for trc in &self.transfer_ring_controllers {
            if trc.is_some() {
                let trc: &Arc<TransferRingController> = trc.as_ref().unwrap();
                trc.stop(auto_callback.clone());
            }
        }
    }

    pub fn stop_endpoint<C: Fn(TrbCompletionCode) + 'static + Send>(&self, endpoint_id: u8, cb: C) {
        if !valid_endpoint_id(endpoint_id) {
            error!("trb indexing wrong endpoint id");
            cb(TrbCompletionCode::TrbError);
            return;
        }
        let index = endpoint_id - 1;
        match self.transfer_ring_controllers[index as usize] {
            Some(ref trc) => {
                debug!("stopping endpoint");
                let auto_cb = AutoCallback::new(move || {
                    cb(TrbCompletionCode::Success);
                });
                trc.stop(auto_cb)
            }
            None => {
                error!("endpoint at index {} is not started", index);
                cb(TrbCompletionCode::ContextStateError);
            }
        }
    }

    pub fn set_tr_dequeue_ptr(&self, endpoint_id: u8, ptr: u64) -> TrbCompletionCode {
        if !valid_endpoint_id(endpoint_id) {
            error!("trb indexing wrong endpoint id");
            return TrbCompletionCode::TrbError;
        }
        let index = endpoint_id - 1;
        match &self.transfer_ring_controllers[index as usize] {
            &Some(ref trc) => {
                trc.set_dequeue_pointer(GuestAddress(ptr));
                return TrbCompletionCode::Success;
            }
            &None => {
                error!("set tr dequeue ptr failed due to no trc started");
                return TrbCompletionCode::ContextStateError;
            }
        }
    }

    fn reset(&mut self) {
        for i in 0..self.transfer_ring_controllers.len() {
            self.transfer_ring_controllers[i] = None;
        }
        debug!("reseting device slot {}!", self.slot_id);
        self.enabled = false;
        self.port_id = 0;
    }

    fn add_one_endpoint(&mut self, device_context_index: u8) {
        debug!(
            "adding one endpoint, device context index {}",
            device_context_index
        );
        let mut device_context = self.get_device_context();
        let transfer_ring_index = (device_context_index - 1) as usize;
        let trc = TransferRingController::new(
            self.mem.clone(),
            self.hub.get_port(self.port_id).unwrap(),
            self.event_loop.clone(),
            self.interrupter.clone(),
            self.slot_id,
            device_context_index,
        );
        trc.set_dequeue_pointer(GuestAddress(
            device_context.endpoint_context[transfer_ring_index].get_tr_dequeue_pointer() << 4,
        ));
        trc.set_consumer_cycle_state(
            device_context.endpoint_context[transfer_ring_index].get_dequeue_cycle_state() > 0,
        );
        self.transfer_ring_controllers[transfer_ring_index] = Some(trc);
        device_context.endpoint_context[transfer_ring_index].set_state(EndpointState::Running);
        self.set_device_context(device_context);
    }

    fn drop_one_endpoint(&mut self, device_context_index: u8) {
        let endpoint_index = (device_context_index - 1) as usize;
        self.transfer_ring_controllers[endpoint_index] = None;
        let mut ctx = self.get_device_context();
        ctx.endpoint_context[endpoint_index].set_state(EndpointState::Disabled);
        self.set_device_context(ctx);
    }

    fn get_device_context(&self) -> DeviceContext {
        self.mem
            .read_obj_from_addr(self.get_device_context_addr())
            .unwrap()
    }

    fn set_device_context(&self, device_context: DeviceContext) {
        self.mem
            .write_obj_at_addr(device_context, self.get_device_context_addr())
            .unwrap();
    }

    fn copy_context(&self, input_context_ptr: GuestAddress, device_context_index: u8) {
        // Note that it could be slot context or device context. They have the same size. Won't
        // make a difference here.
        let ctx: EndpointContext = self
            .mem
            .read_obj_from_addr(
                input_context_ptr
                    .checked_add(
                        (device_context_index as u64 + 1) * DEVICE_CONTEXT_ENTRY_SIZE as u64,
                    )
                    .unwrap(),
            )
            .unwrap();
        debug!("context being copied {:?}", ctx);
        self.mem
            .write_obj_at_addr(
                ctx,
                self.get_device_context_addr()
                    .checked_add(device_context_index as u64 * DEVICE_CONTEXT_ENTRY_SIZE as u64)
                    .unwrap(),
            )
            .unwrap();
    }

    fn get_device_context_addr(&self) -> GuestAddress {
        let addr: u64 = self
            .mem
            .read_obj_from_addr(GuestAddress(
                self.dcbaap.get_value() + size_of::<u64>() as u64 * self.slot_id as u64,
            ))
            .unwrap();
        GuestAddress(addr)
    }

    // Returns the current state of the device slot.
    fn state(&self) -> DeviceSlotState {
        let context = self.get_device_context();
        context.slot_context.state().unwrap()
    }

    fn set_state(&self, state: DeviceSlotState) {
        let mut ctx = self.get_device_context();
        ctx.slot_context.set_state(state);
        self.set_device_context(ctx);
    }
}
