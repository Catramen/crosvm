// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

pub use super::xhci_abi_schema::*;
use data_model::DataInit;
use std;
use std::fmt;

unsafe impl DataInit for Trb {}
unsafe impl DataInit for NormalTrb {}
unsafe impl DataInit for SetupStageTrb {}
unsafe impl DataInit for DataStageTrb {}
unsafe impl DataInit for StatusStageTrb {}
unsafe impl DataInit for IsochTrb {}
unsafe impl DataInit for LinkTrb {}
unsafe impl DataInit for EventDataTrb {}
unsafe impl DataInit for NoopTrb {}
unsafe impl DataInit for DisableSlotCommandTrb {}
unsafe impl DataInit for AddressDeviceCommandTrb {}
unsafe impl DataInit for ConfigureEndpointCommandTrb {}
unsafe impl DataInit for EvaluateContextCommandTrb {}
unsafe impl DataInit for ResetDeviceCommandTrb {}
unsafe impl DataInit for TransferEventTrb {}
unsafe impl DataInit for CommandCompletionEventTrb {}
unsafe impl DataInit for PortStatusChangeEventTrb {}
unsafe impl DataInit for EventRingSegmentTableEntry {}
unsafe impl DataInit for InputControlContext {}
unsafe impl DataInit for SlotContext {}
unsafe impl DataInit for EndpointContext {}

unsafe impl DataInit for DeviceContext {}
unsafe impl DataInit for AddressedTrb {}

unsafe impl TrbCast for Trb {}
unsafe impl TrbCast for NormalTrb {}
unsafe impl TrbCast for SetupStageTrb {}
unsafe impl TrbCast for DataStageTrb {}
unsafe impl TrbCast for StatusStageTrb {}
unsafe impl TrbCast for IsochTrb {}
unsafe impl TrbCast for LinkTrb {}
unsafe impl TrbCast for EventDataTrb {}
unsafe impl TrbCast for NoopTrb {}
unsafe impl TrbCast for DisableSlotCommandTrb {}
unsafe impl TrbCast for AddressDeviceCommandTrb {}
unsafe impl TrbCast for ConfigureEndpointCommandTrb {}
unsafe impl TrbCast for EvaluateContextCommandTrb {}
unsafe impl TrbCast for ResetDeviceCommandTrb {}
unsafe impl TrbCast for TransferEventTrb {}
unsafe impl TrbCast for CommandCompletionEventTrb {}
unsafe impl TrbCast for PortStatusChangeEventTrb {}
unsafe impl TrbCast for EventRingSegmentTableEntry {}
unsafe impl TrbCast for InputControlContext {}
unsafe impl TrbCast for SlotContext {}
unsafe impl TrbCast for EndpointContext {}

/// All trb structs have the same size. One trb could be safely casted to another, though the
/// values might be invalid.
pub unsafe trait TrbCast: DataInit {
    fn cast<T: DataInit + TrbCast>(&self) -> &T {
        match T::from_slice(self.as_slice()) {
            Some(t) => &t,
            _ => panic!("Unable to cast"),
        }
    }
}

impl Trb {
    /// Get trb type.
    pub fn trb_type(&self) -> Option<TrbType> {
        TrbType::from_raw(self.get_trb_type())
    }

    /// Set cyle bit.
    pub fn set_cycle_bit(&mut self, b: bool) {
        match b {
            true => self.set_cycle(1u8),
            false => self.set_cycle(0u8),
        }
    }

    /// Get chain bit.
    pub fn get_chain_bit(&self) -> Option<bool> {
        match self.trb_type().unwrap() {
            TrbType::Normal => Some(self.cast::<NormalTrb>().get_chain() != 0),
            TrbType::DataStage => Some(self.cast::<DataStageTrb>().get_chain() != 0),
            TrbType::StatusStage => Some(self.cast::<StatusStageTrb>().get_chain() != 0),
            TrbType::Isoch => Some(self.cast::<IsochTrb>().get_chain() != 0),
            TrbType::Noop => Some(self.cast::<NoopTrb>().get_chain() != 0),
            TrbType::Link => Some(self.cast::<LinkTrb>().get_chain() != 0),
            TrbType::EventData => Some(self.cast::<EventDataTrb>().get_chain() != 0),
            _ => None,
        }
    }

    /// Get interrupt target.
    pub fn interrupter_target(&self) -> u8 {
        const STATUS_INTERRUPTER_TARGET_OFFSET: u8 = 22;
        (self.get_status() >> STATUS_INTERRUPTER_TARGET_OFFSET) as u8
    }

    /// Only some of trb types could appear in transfer ring.
    pub fn can_be_in_transfer_ring(&self) -> bool {
        match self.trb_type().unwrap() {
            TrbType::Normal
            | TrbType::SetupStage
            | TrbType::DataStage
            | TrbType::StatusStage
            | TrbType::Isoch
            | TrbType::Link
            | TrbType::EventData
            | TrbType::Noop => true,
            _ => false,
        }
    }

    /// Length of this transfer.
    pub fn transfer_length(&self) -> u32 {
        const STATUS_TRANSFER_LENGTH_MASK: u32 = 0x1ffff;
        match self.trb_type().unwrap() {
            TrbType::Normal | TrbType::SetupStage | TrbType::DataStage | TrbType::Isoch => {
                self.get_status() & STATUS_TRANSFER_LENGTH_MASK
            }
            _ => 0,
        }
    }

    /// Returns true if interrupt is required on completion.
    pub fn interrupt_on_completion(&self) -> bool {
        const FLAGS_INTERRUPT_ON_COMPLETION_MASK: u16 = 0x10;
        (self.get_flags() & FLAGS_INTERRUPT_ON_COMPLETION_MASK) > 0
    }

    /// Returns true if this trb is immediate data.
    pub fn immediate_data(&self) -> bool {
        const FLAGS_IMMEDIATE_DATA_MASK: u16 = 0x20;
        match self.trb_type().unwrap() {
            TrbType::Normal | TrbType::SetupStage | TrbType::DataStage | TrbType::Isoch => {
                (self.get_flags() & FLAGS_IMMEDIATE_DATA_MASK) != 0
            }
            _ => false,
        }
    }
}

/// Trait for enum that could be converted from raw u8.
pub trait PrimitiveEnum {
    fn from_raw(val: u8) -> Option<Self>
    where
        Self: std::marker::Sized;
}

/// All kinds of trb.
pub enum TrbType {
    Reserved = 0,
    Normal = 1,
    SetupStage = 2,
    DataStage = 3,
    StatusStage = 4,
    Isoch = 5,
    Link = 6,
    EventData = 7,
    Noop = 8,
    EnableSlotCommand = 9,
    DisableSlotCommand = 10,
    AddressDeviceCommand = 11,
    ConfigureEndpointCommand = 12,
    EvaluateContextCommand = 13,
    ResetDeviceCommand = 17,
    NoopCommand = 23,
    TransferEvent = 32,
    CommandCompletionEvent = 33,
    PortStatusChangeEvent = 34,
}

impl PrimitiveEnum for TrbType {
    fn from_raw(val: u8) -> Option<Self> {
        match val {
            0 => Some(TrbType::Reserved),
            1 => Some(TrbType::Normal),
            2 => Some(TrbType::SetupStage),
            3 => Some(TrbType::DataStage),
            4 => Some(TrbType::StatusStage),
            5 => Some(TrbType::Isoch),
            6 => Some(TrbType::Link),
            7 => Some(TrbType::EventData),
            8 => Some(TrbType::Noop),
            9 => Some(TrbType::EnableSlotCommand),
            10 => Some(TrbType::DisableSlotCommand),
            11 => Some(TrbType::AddressDeviceCommand),
            12 => Some(TrbType::ConfigureEndpointCommand),
            13 => Some(TrbType::EvaluateContextCommand),
            17 => Some(TrbType::ResetDeviceCommand),
            23 => Some(TrbType::NoopCommand),
            32 => Some(TrbType::TransferEvent),
            33 => Some(TrbType::CommandCompletionEvent),
            34 => Some(TrbType::PortStatusChangeEvent),
            _ => None,
        }
    }
}

/// Completion code of trb types.
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
    fn from_raw(val: u8) -> Option<Self> {
        match val {
            1 => Some(TrbCompletionCode::Success),
            4 => Some(TrbCompletionCode::TransactionError),
            5 => Some(TrbCompletionCode::TrbError),
            9 => Some(TrbCompletionCode::NoSlotsAvailableError),
            11 => Some(TrbCompletionCode::SlotNotEnabledError),
            13 => Some(TrbCompletionCode::ShortPacket),
            19 => Some(TrbCompletionCode::ContextStateError),
            _ => None,
        }
    }
}

/// State of device slot.
pub enum DeviceSlotState {
    // The same value (0) is used for both the enabled and disabled states. See
    // xhci spec table 60.
    DisabledOrEnabled = 0,
    Default = 1,
    Addressed = 2,
    Configured = 3,
}

impl PrimitiveEnum for DeviceSlotState {
    fn from_raw(val: u8) -> Option<Self> {
        match val {
            0 => Some(DeviceSlotState::DisabledOrEnabled),
            1 => Some(DeviceSlotState::Default),
            2 => Some(DeviceSlotState::Addressed),
            3 => Some(DeviceSlotState::Configured),
            _ => None,
        }
    }
}

impl SlotContext {
    /// Set slot context state.
    pub fn state(&self) -> Option<DeviceSlotState> {
        DeviceSlotState::from_raw(self.get_slot_state())
    }

    /// Get slot context state.
    pub fn set_state(&mut self, state: DeviceSlotState) {
        self.set_slot_state(state as u8);
    }
}

/// State of endpoint.
pub enum EndpointState {
    Disabled = 0,
    Running = 1,
}

impl PrimitiveEnum for EndpointState {
    fn from_raw(val: u8) -> Option<Self> {
        match val {
            0 => Some(EndpointState::Disabled),
            1 => Some(EndpointState::Running),
            _ => None,
        }
    }
}

impl EndpointContext {
    /// Get endpoint context state.
    pub fn state(&self) -> Option<EndpointState> {
        EndpointState::from_raw(self.get_endpoint_state())
    }

    /// Set endpoint context state.
    pub fn set_state(&mut self, state: EndpointState) {
        self.set_endpoint_state(state as u8);
    }
}

impl InputControlContext {
    /// Get drop context flag.
    pub fn drop_context_flag(&self, idx: u8) -> bool {
        (self.get_drop_context_flags() & (1 << idx)) != 0
    }

    /// Get add context flag.
    pub fn add_context_flag(&self, idx: u8) -> bool {
        (self.get_add_context_flags() & (1 << idx)) != 0
    }
}
