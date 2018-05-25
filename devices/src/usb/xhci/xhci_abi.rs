// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

extern crate bit_field;

use self::bit_field::*;
use std;
use std::fmt;

type B0 = BitField0;
type B1 = BitField1;
type B2 = BitField2;
type B3 = BitField3;
type B4 = BitField4;
type B5 = BitField5;
type B6 = BitField6;
type B7 = BitField7;
type B8 = BitField8;
type B9 = BitField9;
type B10 = BitField10;
type B11 = BitField11;
type B12 = BitField12;
type B13 = BitField13;
type B14 = BitField14;
type B15 = BitField15;
type B16 = BitField16;
type B17 = BitField17;
type B18 = BitField18;
type B19 = BitField19;
type B20 = BitField20;
type B21 = BitField21;
type B22 = BitField22;
type B23 = BitField23;
type B24 = BitField24;
type B25 = BitField25;
type B26 = BitField26;
type B27 = BitField27;
type B28 = BitField28;
type B29 = BitField29;
type B30 = BitField30;
type B31 = BitField31;
type B32 = BitField32;
type B33 = BitField33;
type B34 = BitField34;
type B35 = BitField35;
type B36 = BitField36;
type B37 = BitField37;
type B38 = BitField38;
type B39 = BitField39;
type B40 = BitField40;
type B41 = BitField41;
type B42 = BitField42;
type B43 = BitField43;
type B44 = BitField44;
type B45 = BitField45;
type B46 = BitField46;
type B47 = BitField47;
type B48 = BitField48;
type B49 = BitField49;
type B50 = BitField50;
type B51 = BitField51;
type B52 = BitField52;
type B53 = BitField53;
type B54 = BitField54;
type B55 = BitField55;
type B56 = BitField56;
type B57 = BitField57;
type B58 = BitField58;
type B59 = BitField59;
type B60 = BitField60;
type B61 = BitField61;
type B62 = BitField62;
type B63 = BitField63;
type B64 = BitField64;

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
