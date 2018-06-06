// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

unsafe impl data_model::DataInit for Trb {}

pub enum Error {
    InvalidValue(u8),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::InvalidValue(val) => write!(f, "Primitive Enum got an invaild value: {}", val),
        }
    }
}

type Result<T> = std::result::Result<T, Error>;

pub trait PrimitiveEnum {
    fn from(val: u8) -> Result<Self>
    where
        Self: std::marker::Sized;
    fn to(&self) -> u8;
}

pub enum TrbType {
    Reserved,
    Normal,
    SetupStage,
    DataStage,
    StatusStage,
    Isoch,
    Link,
    EventData,
    Noop,
    EnableSlotCommand,
    DisableSlotCommand,
    AddressDeviceCommand,
    ConfigureEndpointCommand,
    EvaluateContextCommand,
    ResetDeviceCommand,
    NoopCommand,
    TransferEvent,
    CommandCompletionEvent,
    PortStatusChangeEvent,
}

impl PrimitiveEnum for TrbType {
    fn from(val: u8) -> Result<Self> {
        match val {
            0 => Ok(TrbType::Reserved),
            1 => Ok(TrbType::Normal),
            2 => Ok(TrbType::SetupStage),
            3 => Ok(TrbType::DataStage),
            4 => Ok(TrbType::StatusStage),
            5 => Ok(TrbType::Isoch),
            6 => Ok(TrbType::Link),
            7 => Ok(TrbType::EventData),
            8 => Ok(TrbType::Noop),
            9 => Ok(TrbType::EnableSlotCommand),
            10 => Ok(TrbType::DisableSlotCommand),
            11 => Ok(TrbType::AddressDeviceCommand),
            12 => Ok(TrbType::ConfigureEndpointCommand),
            13 => Ok(TrbType::EvaluateContextCommand),
            17 => Ok(TrbType::ResetDeviceCommand),
            23 => Ok(TrbType::NoopCommand),
            32 => Ok(TrbType::TransferEvent),
            33 => Ok(TrbType::CommandCompletionEvent),
            34 => Ok(TrbType::PortStatusChangeEvent),
            _ => Err(Error::InvalidValue(val)),
        }
    }
    fn to(&self) -> u8 {
        match self {
            &TrbType::Reserved => 0,
            &TrbType::Normal => 1,
            &TrbType::SetupStage => 2,
            &TrbType::DataStage => 3,
            &TrbType::StatusStage => 4,
            &TrbType::Isoch => 5,
            &TrbType::Link => 6,
            &TrbType::EventData => 7,
            &TrbType::Noop => 8,
            &TrbType::EnableSlotCommand => 9,
            &TrbType::DisableSlotCommand => 10,
            &TrbType::AddressDeviceCommand => 11,
            &TrbType::ConfigureEndpointCommand => 12,
            &TrbType::EvaluateContextCommand => 13,
            &TrbType::ResetDeviceCommand => 17,
            &TrbType::NoopCommand => 23,
            &TrbType::TransferEvent => 32,
            &TrbType::CommandCompletionEvent => 33,
            &TrbType::PortStatusChangeEvent => 34,
        }
    }
}

pub enum TrbCompletionCode {
    Success = 1,
    TransactionError = 4,
    TrbError = 5,
    NoSlotsAvailableError = 9,
    SlotNotEnabledError = 11,
    ShortPacket = 13,
    ContextStateError = 19,
}

impl PrimitiveEnum for TrbCompletionCode {
    fn from(val: u8) -> Result<Self> {
        match val {
            1 => Ok(TrbCompletionCode::Success),
            4 => Ok(TrbCompletionCode::TransactionError),
            5 => Ok(TrbCompletionCode::TrbError),
            9 => Ok(TrbCompletionCode::NoSlotsAvailableError),
            11 => Ok(TrbCompletionCode::SlotNotEnabledError),
            13 => Ok(TrbCompletionCode::ShortPacket),
            19 => Ok(TrbCompletionCode::ContextStateError),
            _ => Err(Error::InvalidValue(val)),
        }
    }

    fn to(&self) -> u8 {
        match self {
            &TrbCompletionCode::Success => 1,
            &TrbCompletionCode::TransactionError => 4,
            &TrbCompletionCode::TrbError => 5,
            &TrbCompletionCode::NoSlotsAvailableError => 9,
            &TrbCompletionCode::SlotNotEnabledError => 11,
            &TrbCompletionCode::ShortPacket => 13,
            &TrbCompletionCode::ContextStateError => 19,
        }
    }
}

pub enum DeviceSlotState {
    // The same value (0) is used for both the enabled and disabled states. See
    // xhci spec table 60.
    DisabledOrEnabled,
    Default,
    Addressed,
    Configured,
}

impl PrimitiveEnum for DeviceSlotState {
    fn from(val: u8) -> Result<Self> {
        match val {
            0 => Ok(DeviceSlotState::DisabledOrEnabled),
            1 => Ok(DeviceSlotState::Default),
            2 => Ok(DeviceSlotState::Addressed),
            3 => Ok(DeviceSlotState::Configured),
            _ => Err(Error::InvalidValue(val)),
        }
    }

    fn to(&self) -> u8 {
        match self {
            &DeviceSlotState::DisabledOrEnabled => 0,
            &DeviceSlotState::Default => 1,
            &DeviceSlotState::Addressed => 2,
            &DeviceSlotState::Configured => 3,
        }
    }
}

pub enum EndpointState {
    Disabled = 0,
    Running = 1,
}

impl PrimitiveEnum for EndpointState {
    fn from(val: u8) -> Result<Self> {
        match val {
            0 => Ok(EndpointState::Disabled),
            1 => Ok(EndpointState::Running),
            _ => Err(Error::InvalidValue(val)),
        }
    }

    fn to(&self) -> u8 {
        match self {
            &EndpointState::Disabled => 0,
            &EndpointState::Running => 1,
        }
    }
}

impl Trb {
    pub fn trb_type(&self) -> Result<TrbType> {
        TrbType::from(self.get_trb_type());
    }

    pub fn set_cycle_bit(&mut self, b: bool) {
        match b {
            true => self.set_cycle(1u8),
            false => self.set_cyle(0u8),
        }
    }
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct NormalTrbSchema {
    data_buffer: B64,
    trb_transfer_length: B17,
    td_size: B5,
    interrupter_target: B10,
    cycle: B1,
    evaluate_next_trb: B1,
    interrupt_on_short_packet: B1,
    no_snoop: B1,
    chain: B1,
    interrupt_on_completion: B1,
    immediate_data: B1,
    reserved: B2,
    block_event_interrupt: B1,
    trb_type: B6,
    reserved1: B16,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]

pub struct SetupStageTrbSchema {
    request_type: B8,
    request: B8,
    value: B16,
    index: B16,
    length: B16,
    trb_transfer_length: B17,
    reserved0: B5,
    interrupter_target: B10,
    cycle: B1,
    reserved1: B4,
    interrupt_on_completion: B1,
    immediate_data: B1,
    reserved2: B3,
    trb_type: B6,
    transfer_type: B2,
    reserved3: B14,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]

pub struct DataStageTrbSchema {
    data_buffer_pointer: B64,
    trb_transfer_length: B17,
    td_size: B5,
    interrupter_target: B10,
    cycle: B1,
    evaluate_next_trb: B1,
    interrupt_on_short_packet: B1,
    no_snoop: B1,
    chain: B1,
    interrupt_on_completion: B1,
    immediate_data: B1,
    reserved0: B3,
    trb_type: B6,
    direction: B1,
    reserved1: B15,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct StatusStageTrbSchema {
    reserved0: B64,
    reserved1: B22,
    interrupter_target: B10,
    cycle: B1,
    evaluate_next_trb: B1,
    reserved2: B2,
    chain: B1,
    interrupt_on_completion: B1,
    reserved3: B4,
    trb_type: B6,
    direction: B1,
    reserved4: B15,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct IsochTrbSchema {
    data_buffer_pointer: B64,
    trb_transfer_length: B17,
    td_size: B5,
    interrupter_target: B10,
    cycle: B1,
    evaulate_nex_trb: B1,
    interrupt_on_short_packet: B1,
    no_snoop: B1,
    chain: B1,
    interrupt_on_completion: B1,
    immediate_data: B1,
    transfer_burst_count: B2,
    block_event_interrupt: B1,
    trb_type: B6,
    tlbpc: B4,
    frame_id: B11,
    sia: B1,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct LinkTrbSchema {
    ring_segment_pointer: B64,
    reserved0: B22,
    interrupter_target: B10,
    cycle: B1,
    toggle_cycle: B1,
    reserved1: B2,
    chain: B1,
    interrupt_on_completion: B1,
    reserved2: B4,
    trb_type: B6,
    reserved3: B16,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct EventDataTrbSchema {
    event_data: B64,
    reserved0: B22,
    interrupter_target: B10,
    cycle: B1,
    evaluate_next_trb: B1,
    reserved1: B2,
    chain: B1,
    interrupt_on_completion: B1,
    reserved2: B3,
    block_event_interrupt: B1,
    trb_type: B6,
    reserved3: B16,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct NoopTrbSchema {
    reserved0: B64,
    reserved1: B22,
    interrupter_target: B10,
    cycle: B1,
    evaluate_next_trb: B1,
    reserved2: B2,
    chain: B1,
    interrupt_on_completion: B1,
    reserved3: B4,
    trb_type: B6,
    reserved4: B16,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct DisableSlotCommandTrbSchema {
    reserved0: B32,
    reserved1: B32,
    reserved2: B32,
    cycle: B1,
    reserved3: B9,
    trb_type: B6,
    reserved4: B8,
    slot_id: B8,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct AddressDeviceCommandTrbSchema {
    input_context_pointer: B64,
    reserved: B32,
    cycle: B1,
    reserved2: B8,
    block_set_address_request: B1,
    trb_type: B6,
    reserved3: B8,
    slot_id: B8,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct ConfigureEndpointCommandTrbSchema {
    input_context_pointer: B64,
    reserved0: B32,
    cycle: B1,
    reserved1: B8,
    deconfigure: B1,
    trb_type: B6,
    reserved2: B8,
    slot_id: B8,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct EvaluateContextCommandTrbSchema {
    input_context_pointer: B64,
    reserved0: B32,
    cycle: B1,
    reserved1: B9,
    trb_type: B6,
    reserved2: B8,
    slot_id: B8,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct ResetDeviceCommandTrbSchema {
    reserved0: B32,
    reserved1: B32,
    reserved2: B32,
    cycle: B1,
    reserved3: B9,
    trb_type: B6,
    reserved4: B8,
    slot_id: B8,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct TransferEventTrbSchema {
    trb_pointer: B64,
    trb_transfer_length: B24,
    completion_code: B8,
    cycle: B1,
    reserved0: B1,
    event_data: B1,
    reserved1: B7,
    trb_type: B6,
    endpoint_id: B5,
    reserved2: B3,
    slot_id: B8,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct CommandCompletionEventTrbSchema {
    trb_pointer: B64,
    command_completion_parameter: B24,
    completion_code: B8,
    cycle: B1,
    reserved: B9,
    trb_type: B6,
    vf_id: B8,
    slot_id: B8,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct PortStatusChangeEventTrbSchema {
    reserved0: B24,
    port_id: B8,
    reserved1: B32,
    reserved2: B24,
    completion_code: B8,
    cycle: B1,
    reserved3: B9,
    trb_type: B6,
    reserved4: B16,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct EventRingSegmentTableEntrySchema {
    ring_segment_base_address: B64,
    ring_segment_size: B16,
    reserved2: B48,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct InputControlContextSchema {
    // Xhci spec 6.2.5.1.
    drop_context_flags: B32,
    add_context_flags: B32,
    reserved0: B32,
    reserved1: B32,
    reserved2: B32,
    reserved3: B32,
    reserved4: B32,
    configuration_value: B8,
    interface_number: B8,
    alternate_setting: B8,
    reserved5: B8,
}

// Size for device context entries (SlotContext and EndpointContext).
const DEVICE_CONTEXT_ENTRY_SIZE: usize = 32usize;

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct SlotContextSchema {
    route_string: B20,
    speed: B4,
    reserved1: B1,
    mtt: B1,
    hub: B1,
    context_entries: B5,
    max_exit_latency: B16,
    root_hub_port_number: B8,
    num_ports: B8,
    tt_hub_slot_id: B8,
    tt_port_number: B8,
    tt_think_time: B2,
    reserved2: B4,
    interrupter_target: B10,
    usb_device_address: B8,
    reserved3: B19,
    slot_state: B5,
    reserved4: B32,
    reserved5: B32,
    reserved6: B32,
    reserved7: B32,
}

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct EndpointContextSchema {
    endpoint_state: B3,
    reserved1: B5,
    mult: B2,
    max_primary_streams: B5,
    linear_stream_array: B1,
    interval: B8,
    max_esit_payload_hi: B8,
    reserved2: B1,
    error_count: B2,
    endpoint_type: B3,
    reserved3: B1,
    host_initiate_disable: B1,
    max_burst_size: B8,
    max_packet_size: B16,
    dequeue_cycle_state: B1,
    reserved4: B3,
    tr_dequeue_pointer: B60,
    average_trb_length: B16,
    max_esit_payload_lo: B16,
    reserved5: B32,
    reserved6: B32,
    reserved7: B32,
}

pub struct DeviceContext {
    slot_context: SlotContext,
    endpoint_context: [EndpointContext; 31],
}

// POD struct for associating a TRB with its address in guest memory.  This is
// useful because transfer and command completion event TRBs must contain
// pointers to the original TRB that generated the event.
pub struct AddressedTrb {
    trb: Trb,
    gpa: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_struct_sizes() {
        assert_eq!(std::mem::size_of::<Trb>(), TRB_SIZE);
        assert_eq!(std::mem::size_of::<NormalTrb>(), TRB_SIZE);
        assert_eq!(std::mem::size_of::<SetupStageTrb>(), TRB_SIZE);
        assert_eq!(std::mem::size_of::<DataStageTrb>(), TRB_SIZE);
        assert_eq!(std::mem::size_of::<StatusStageTrb>(), TRB_SIZE);
        assert_eq!(std::mem::size_of::<IsochTrb>(), TRB_SIZE);
        assert_eq!(std::mem::size_of::<LinkTrb>(), TRB_SIZE);
        assert_eq!(std::mem::size_of::<EventDataTrb>(), TRB_SIZE);
        assert_eq!(std::mem::size_of::<NoopTrb>(), TRB_SIZE);
        assert_eq!(std::mem::size_of::<DisableSlotCommandTrb>(), TRB_SIZE);
        assert_eq!(std::mem::size_of::<AddressDeviceCommandTrb>(), TRB_SIZE);
        assert_eq!(std::mem::size_of::<ConfigureEndpointCommandTrb>(), TRB_SIZE);
        assert_eq!(std::mem::size_of::<EvaluateContextCommandTrb>(), TRB_SIZE);
        assert_eq!(std::mem::size_of::<ResetDeviceCommandTrb>(), TRB_SIZE);
        assert_eq!(std::mem::size_of::<TransferEventTrb>(), TRB_SIZE);
        assert_eq!(std::mem::size_of::<CommandCompletionEventTrb>(), TRB_SIZE);
        assert_eq!(std::mem::size_of::<PortStatusChangeEventTrb>(), TRB_SIZE);

        assert_eq!(std::mem::size_of::<EventRingSegmentTableEntry>(), 16);
        assert_eq!(std::mem::size_of::<InputControlContext>(), 32);
        assert_eq!(std::mem::size_of::<SlotContext>(), DEVICE_CONTEXT_ENTRY_SIZE);
        assert_eq!(std::mem::size_of::<EndpointContext>(), DEVICE_CONTEXT_ENTRY_SIZE);
        assert_eq!(std::mem::size_of::<DeviceContext>(), 32 * DEVICE_CONTEXT_ENTRY_SIZE);
    }
}
