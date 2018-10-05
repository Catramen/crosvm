// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::interrupter::Interrupter;
use super::mmio_register::Register;
use super::transfer_ring_controller::TransferRingController;
use super::usb_ports::UsbPorts;
use super::xhci_abi::{
    AddressDeviceCommandTrb, AddressedTrb, ConfigureEndpointCommandTrb, DeviceContext,
    DeviceSlotState, EndpointContext, EndpointState, EvaluateContextCommandTrb,
    InputControlContext, SlotContext, TrbCompletionCode, DEVICE_CONTEXT_ENTRY_SIZE,
};
use super::xhci_backend_device::XhciBackendDevice;
use super::xhci_regs::MAX_SLOTS;
use std::mem::size_of;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::sync::{Arc, Mutex, MutexGuard};
use sys_util::{EventFd, GuestAddress, GuestMemory};
use usb::auto_callback::AutoCallback;
use usb::event_loop::{EventHandler, EventLoop};

/// 0: Control endpoint
/// 1: Endpoint 1 out
/// 2: Endpoint 1 in
/// 3: Endpoint 2 out
/// ...
/// 30: Endpoint 15 in
pub const TOTAL_TRANSFER_RING_CONTROLLERS: usize = 31;

#[derive(Clone)]
pub struct DeviceSlots {
    ports: Arc<Mutex<UsbPorts>>,
    slots: Vec<Arc<Mutex<DeviceSlot>>>,
}

impl DeviceSlots {
    pub fn new(
        dcbaap: Register<u64>,
        ports: Arc<Mutex<UsbPorts>>,
        interrupter: Arc<Mutex<Interrupter>>,
        event_loop: EventLoop,
        mem: GuestMemory,
    ) -> DeviceSlots {
        let mut vec = Vec::new();
        for i in 0..MAX_SLOTS {
            vec.push(Arc::new(Mutex::new(DeviceSlot::new(
                (i + 1) as u8,
                dcbaap.clone(),
                ports.clone(),
                interrupter.clone(),
                event_loop.clone(),
                mem.clone(),
            ))));
        }
        DeviceSlots {
            ports: ports,
            slots: vec,
        }
    }

    pub fn slot(&self, slot_id: u8) -> Option<MutexGuard<DeviceSlot>> {
        if slot_id >= MAX_SLOTS as u8 {
            None
        } else {
            Some(self.slots[slot_id as usize].lock().unwrap())
        }
    }

    pub fn ports(&self) -> &Arc<Mutex<UsbPorts>> {
        &self.ports
    }

    pub fn stop_all_and_reset<C: Fn() + 'static + Send>(&self, callback: C) {
        let slots = self.slots.clone();
        let ports = self.ports.clone();
        let auto_callback = AutoCallback::new(move || {
            for slot in &slots {
                slot.lock().unwrap().reset();
                ports.lock().unwrap().reset();
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

    pub fn disable_slot(&self, slot_id: u8, atrb: &AddressedTrb, event_fd: EventFd) {
        DeviceSlot::disable(&self.slots[slot_id as usize], atrb, event_fd);
    }

    pub fn reset_slot(&self, slot_id: u8, atrb: &AddressedTrb, event_fd: EventFd) {
        DeviceSlot::reset_slot(&self.slots[slot_id as usize], atrb, event_fd);
    }
}

pub struct DeviceSlot {
    slot_id: u8,
    dcbaap: Register<u64>,
    ports: Arc<Mutex<UsbPorts>>,
    interrupter: Arc<Mutex<Interrupter>>,
    event_loop: EventLoop,
    mem: GuestMemory,
    enabled: bool,
    backend: Option<Arc<Mutex<XhciBackendDevice>>>,
    transfer_ring_controllers: Vec<Option<Arc<TransferRingController>>>,
}

impl DeviceSlot {
    pub fn new(
        slot_id: u8,
        dcbaap: Register<u64>,
        ports: Arc<Mutex<UsbPorts>>,
        interrupter: Arc<Mutex<Interrupter>>,
        event_loop: EventLoop,
        mem: GuestMemory,
    ) -> Self {
        let mut transfer_ring_controllers = Vec::new();
        for i in 0..TOTAL_TRANSFER_RING_CONTROLLERS {
            transfer_ring_controllers.push(None);
        }
        DeviceSlot {
            slot_id,
            dcbaap,
            ports,
            interrupter,
            event_loop,
            mem,
            enabled: false,
            backend: None,
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
    pub fn ring_doorbell(&self, target: usize, stream_id: u16) -> bool {
        if target < 1 || target > 31 {
            error!(
                "Invalid target written to doorbell register. target: {}",
                target
            );
            return false;
        }

        let i = target - 1;
        let transfer_ring_controller = match self.transfer_ring_controllers[i].as_ref() {
            Some(tr) => tr,
            None => {
                error!("Device endpoint is not inited");
                return false;
            }
        };
        let context = self.get_device_context();
        if context.endpoint_context[target].get_endpoint_state() == EndpointState::Running as u8 {
            transfer_ring_controller.start();
        }
        return true;
    }

    /// Enable the slot, return if it's successful.
    pub fn enable(&mut self) -> bool {
        if self.enabled {
            return false;
        }

        // Initialize the control endpoint.
        self.transfer_ring_controllers[0] = Some(TransferRingController::new(
            self.mem.clone(),
            self.event_loop.clone(),
            self.interrupter.clone(),
            self.slot_id,
            0,
            self.backend.as_ref().unwrap().clone(),
        ));
        self.enabled = true;
        return true;
    }

    pub fn disable(slot: &Arc<Mutex<DeviceSlot>>, atrb: &AddressedTrb, event_fd: EventFd) {
        let s = slot.lock().unwrap();
        let gpa = atrb.gpa;
        let slot_id = s.slot_id;
        if s.enabled {
            let interrupter = s.interrupter.clone();
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
                interrupter.lock().unwrap().send_command_completion_trb(
                    TrbCompletionCode::Success,
                    slot_id,
                    GuestAddress(gpa),
                );
                event_fd.write(1).unwrap();
            });
            s.stop_all_trc(auto_callback);
        } else {
            s.interrupter.lock().unwrap().send_command_completion_trb(
                TrbCompletionCode::SlotNotEnabledError,
                slot_id,
                GuestAddress(gpa),
            );
            event_fd.write(1).unwrap();
        }
    }

    // Assigns the device address and initializes slot and endpoint 0 context.
    pub fn set_address(&mut self, trb: &AddressDeviceCommandTrb) -> TrbCompletionCode {
        if !self.enabled {
            return TrbCompletionCode::SlotNotEnabledError;
        }
        let mut device_context = self.get_device_context();
        if (device_context.slot_context.state().unwrap() != DeviceSlotState::DisabledOrEnabled)
            && (device_context.slot_context.state().unwrap() != DeviceSlotState::Default
                || trb.get_block_set_address_request() > 0)
        {
            return TrbCompletionCode::ContextStateError;
        }

        // Copy all fields of the slot context and endpoint 0 context from the input context
        // to the output context.
        let input_context_ptr = GuestAddress(trb.get_input_context_pointer());
        self.copy_context(input_context_ptr, 0);
        self.copy_context(input_context_ptr, 1);

        self.backend = self
            .ports
            .lock()
            .unwrap()
            .get_backend_for_port(device_context.slot_context.get_root_hub_port_number());
        // Assign slot ID as device address if block_set_address_request is not set.
        if trb.get_block_set_address_request() > 0 {
            device_context
                .slot_context
                .set_state(DeviceSlotState::Default);
        } else {
            if self.backend.is_some() {
                self.backend
                    .as_ref()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .set_address(self.slot_id as u32);
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

        device_context.endpoint_context[0].set_state(EndpointState::Running);
        self.set_device_context(device_context);
        TrbCompletionCode::Success
    }

    // Adds or dropbs multiple endpoints in the device slot.
    pub fn configure_endpoint(&mut self, trb: &ConfigureEndpointCommandTrb) -> TrbCompletionCode {
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
                self.add_one_endpoint(
                    GuestAddress(trb.get_input_context_pointer()),
                    device_context_index,
                );
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

        let mut device_context = self.get_device_context();
        let state = device_context.slot_context.state().unwrap();
        if state == DeviceSlotState::Default
            || state == DeviceSlotState::Addressed
            || state == DeviceSlotState::Configured
        {
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
    pub fn reset_slot(slot: &Arc<Mutex<DeviceSlot>>, atrb: &AddressedTrb, event_fd: EventFd) {
        let s = slot.lock().unwrap();
        let gpa = atrb.gpa;
        let state = s.state();
        if state != DeviceSlotState::Addressed && state != DeviceSlotState::Configured {
            s.interrupter.lock().unwrap().send_command_completion_trb(
                TrbCompletionCode::ContextStateError,
                s.slot_id,
                GuestAddress(gpa),
            );
            event_fd.write(1).unwrap();
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
            s.interrupter.lock().unwrap().send_command_completion_trb(
                TrbCompletionCode::Success,
                s.slot_id,
                GuestAddress(gpa),
            );
            event_fd.write(1).unwrap();
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

    fn reset(&mut self) {
        for i in 0..self.transfer_ring_controllers.len() {
            self.transfer_ring_controllers[i] = None;
        }
        self.enabled = false;
        self.backend = None;
    }

    fn add_one_endpoint(&mut self, input_context_ptr: GuestAddress, device_context_index: u8) {
        let mut device_context = self.get_device_context();
        let transfer_ring_index = (device_context_index - 1) as usize;
        let trc = TransferRingController::new(
            self.mem.clone(),
            self.event_loop.clone(),
            self.interrupter.clone(),
            self.slot_id,
            device_context_index,
            self.backend.as_ref().unwrap().clone(),
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
            .write_obj_at_addr(device_context, self.get_device_context_addr());
    }

    fn copy_context(&self, input_context_ptr: GuestAddress, device_context_index: u8) {
        let ctx: EndpointContext = self.mem.read_obj_from_addr(input_context_ptr).unwrap();
        self.mem.write_obj_at_addr(
            ctx,
            self.get_device_context_addr()
                .checked_add(device_context_index as u64 * DEVICE_CONTEXT_ENTRY_SIZE as u64)
                .unwrap(),
        );
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

    // Returns th ecuurent state of the device slot.
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
