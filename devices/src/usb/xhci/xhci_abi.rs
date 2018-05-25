// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

extern crate bit_field;

#[macro_use]
use bit_field::*;

type b1 = BitField1;
type b2 = BitField2;
type b3 = BitField3;
type b4 = BitField4;
type b5 = BitField5;
type b64 = BitField64;

// Bitmasks for the usbcmd register.
const USB_CMD_RUNSTOP: u32 = 1u32 << 0;
const USB_CMD_RESET: u32 = 1u32 << 1;
const USB_CMD_INTERRUPTER_ENABLE: u32 = 1u32 << 2;

// Bitmasks for the usbsts register.
const USB_STS_HALTED: u32 = 1u32 << 0;
const USB_STS_EVENT_INTERRUPT: u32 = 1u32 << 3;
const USB_STS_PORT_CHANGE_DETECT: u32 = 1u32 << 4;
const USB_STS_CONTROLLER_NOT_READY: u32 = 1u32 << 11;
const USB_STS_SET_TO_CLEAR_MASK: u32 = 0x0000041C;

// Bitmasks for the crcr register.
const CRCR_RING_CYCLE_STATE: u64 = 1u64 << 0;
const CRCR_COMMAND_STOP: u64 = 1u64 << 1;
const CRCR_COMMAND_ABORT: u64 = 1u64 << 2;
const CRCR_COMMAND_RING_RUNNING: u64 = 1u64 << 3;
const CRCR_COMMAND_RING_POINTER: u64 = 0xFFFFFFFFFFFFFFC0;

// Bitmasks for portsc registers.
const PORTSC_CURRENT_CONNECT_STATUS: u32 = 1u32 << 0;
const PORTSC_PORT_ENABLED: u32 = 1u32 << 1;
const PORTSC_PORT_RESET: u32 = 1u32 << 4;
const PORTSC_PORT_LINK_STATE_MASK: u32 = 0x000001E0;
const PORTSC_PORT_POWER: u32 = 1u32 << 9;
const PORTSC_CONNECT_STATUS_CHANGE: u32 = 1u32 << 17;
const PORTSC_PORT_ENABLED_DISABLED_CHANGE: u32 = 1u32 << 18;
const PORTSC_PORT_RESET_CHANGE: u32 = 1u32 << 21;
const PORTSC_WARM_PORT_RESET: u32 = 1u32 << 31;
const PORTSC_SET_TO_CLEAR_MASK: u32 = 0x00FE0002;

// Bitmasks for iman registers.
const IMAN_INTERRUPT_PENDING: u32 = 1u32 << 0;
const IMAN_INTERRUPT_ENABLE: u32 = 1u32 << 1;
const IMAN_SET_TO_CLEAR_MASK: u32 = 0x00000001;

// Bitmasks and offsets for imod registers.
const IMOD_INTERRUPT_MODERATION_INTERVAL: u32 = 0xFFFF;
const IMOD_INTERRUPT_MODERATION_COUNTER_OFFSET: u8 = 16;

// Bitmasks for erstsz registers.
const ERSTSZ_SEGMENT_TABLE_SIZE: u32 = 0xFFFF;

// Bitmasks for erstba registers.
const ERSTBA_SEGMENT_TABLE_BASE_ADDRESS: u64 = 0xFFFFFFFFFFFFFFC0;

// Bitmasks for erdp registers.
const ERDP_EVENT_HANDLER_BUSY: u64 = 1u64 << 3;
const ERDP_EVENT_RING_DEQUEUE_POINTER: u64 = 0xFFFFFFFFFFFFFFF0;
const ERDP_SET_TO_CLEAR_MASK: u64 = 0x0000000000000008;

// Bitmasks and offsets for doorbell registers.
const DOORBELL_TARGET: u32 = 0xFF;
const DOORBELL_STREAM_ID_OFFSET: u32 = 16;

// Bitmasks and offsets for structural parameter registers.
const HCSPARAMS1_MAX_INTERRUPTERS_MASK: u32 = 0x7FF00;
const HCSPARAMS1_MAX_INTERRUPTERS_OFFSET: u32 = 8;
const HCSPARAMS1_MAX_SLOTS_MASK: u32 = 0xFF;

// Bitmasks and offsets for extended capabilities registers.
const SPCAP_PORT_COUNT_MASK: u32 = 0xFF00;
const SPCAP_PORT_COUNT_OFFSET: u32 = 8;

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
    fn from(u8: val) -> Result<Self>;
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
    fn from(u8: val) -> Result<Self> {
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
            _ => Err(Error::InvalidValue),
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
    fn from(u8: val) -> Result<Self> {
        match val {
            1 => Ok(TrbCompletionCode::Success),
            4 => Ok(TrbCompletionCode::TransactionError),
            5 => Ok(TrbCompletionCode::TrbError),
            9 => Ok(TrbCompletionCode::NoSlotsAvailableError),
            11 => Ok(TrbCompletionCode::SlotNotEnabledError),
            13 => Ok(TrbCompletionCode::ShortPacket),
            19 => Ok(TrbCompletionCode::ContextStateError),
            _ => Err(Error::InvalidValue),
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
    StateAddressed,
    Configured,
}

impl PrimitiveEnum for DeviceSlotState {
    fn from(u8: val) -> Result<Self> {
        match val {
            0 => Ok(DeviceSlotState::DisabledOrEnabled),
            1 => Ok(DeviceSlotState::Default),
            2 => Ok(DeviceSlotState::Addressed),
            3 => Ok(DeviceSlotState::Configured),
            _ => Err(Error::InvalidValue),
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
    fn from(u8: val) -> Result<Self> {
        match val {
            0 => Ok(EndpointState::Disabled),
            1 => Ok(EndpointState::Running),
            _ => Err(Error::InvalidValue),
        }
    }

    fn to(&self) -> u8 {
        match self {
            &EndpointState::Disabled => 0,
            &EndpointState::Running => 1,
        }
    }
}

// Fixed size for all TRB types.
const TRB_SIZE: usize = 16;

// Generic TRB struct containing only fields common to all types.
// TODO(jkwang) add stringify.

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]
pub struct TrbSchema {
    parameter: B64,
    status: B32,
    cycle: B1,
    flags: B9,
    trb_type: B6,
    control: B16,
}

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

pub struct IsochTrbSchema {
    data_buffer_pointer: B64,
    trb_transfer_length: B17,
    td_size: B5,
    interrupter_target: B10,
    cycle: B1,
    evaulate_nex_trb: B1,
    interrupt_on_short_packet: B1,
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

pub struct EvaluateContextCommandTrbSchema {
    input_context_pointer: B64,
    reserved0: B32,
    cycle: B1,
    reserved1: B9,
    trb_type: B6,
    reserved2: B8,
    slot_id: B8,
}

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

pub struct EventRingSegmentTableEntrySchema {
    ring_segment_base_address: B64,
    ring_segment_size: B16,
    reserved2: B48,
}

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
    gpa: GuestPhysicalAddress,
}
