// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

extern crate bit_field;

#[macro_use]
use bit_field::*;

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
const ERDP_EVENT_HANDLER_BUSY: u64 = 1ULL << 3;
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

// Fixed size for all TRB types.
const TRB_SIZE: usize = 16;

pub enum Error {
    InvalidValue(u8),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result{
        match self {
            &Error::InvalidValue(val) => write!(f, "Primitive Enum got an invaild value: {}", val),
        }
    }
}

type Result<T> std::result::Result<T, Error>;

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
            _ => Err(Error::InvalidValue);
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
};

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
};

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
#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]

