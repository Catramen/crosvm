// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

unsafe impl data_model::DataInit for Trb {}
unsafe impl data_model::DataInit for NormalTrb {}
unsafe impl data_model::DataInit for SetupStageTrb {}
unsafe impl data_model::DataInit for DataStageTrb {}
unsafe impl data_model::DataInit for StatusStageTrb {}
unsafe impl data_model::DataInit for IsochTrb {}
unsafe impl data_model::DataInit for LinkTrb {}
unsafe impl data_model::DataInit for EventDataTrb {}
unsafe impl data_model::DataInit for NoopTrb {}
unsafe impl data_model::DataInit for DisableSlotCommandTrb {}
unsafe impl data_model::DataInit for AddressDeviceCommandTrb {}
unsafe impl data_model::DataInit for ConfigureEndpointCommandTrb {}
unsafe impl data_model::DataInit for EvaluateContextCommandTrb {}
unsafe impl data_model::DataInit for ResetDeviceCommandTrb {}
unsafe impl data_model::DataInit for TransferEventTrb {}
unsafe impl data_model::DataInit for CommandCompletionEventTrb {}
unsafe impl data_model::DataInit for PortStatusChangeEventTrb {}
unsafe impl data_model::DataInit for EventRingSegmentTableEntry {}
unsafe impl data_model::DataInit for InputControlContext {}
unsafe impl data_model::DataInit for SlotContext {}
unsafe impl data_model::DataInit for EndpointContext {}

unsafe impl data_model::DataInit for DeviceContext {}
unsafe impl data_model::DataInit for AddressedTrb {}

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

// One trb could be safely casted to another.
pub trait TrbCast: DataInit {
    fn cast<T: DataInit + TrbCast>(&self) -> &T {
        match T::from_slice(self.as_slice()) {
            Some(&t) => t,
            _ => panic!("Unable to cast"),
        }
    }
}

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

    pub fn get_chain_bit(&self) -> bool {
        match self.trb_type().unwrap() {
            TrbType::Normal => self.cast<NormalTrb>().get_chain() != 0,
            TrbType::DataStage => self.cast<DataStageTrb>().get_chain() != 0,
            TrbType::StatusStage => self.cast<StatusStageTrb>().get_chain() != 0,
            TrbType::Isoch => self.cast<IsochTrb>().get_chain() != 0,
            TrbType::Noop => self.cast<NoopTrb>().get_chain() != 0,
            TrbType::Link => self.cast<LinkTrb>().get_chain() != 0,
            TrbType::EventData => self.cast<EventDataTrb>().get_chain() != 0,
            // TODO(jkwang) add log here.
            _ => false,
        }
    }

    pub fn interrupter_target(&self) -> u8 {
        const STATUS_INTERRUPTER_TARGET_OFFSET: u8 = 22;
        self.get_status() >> STATUS_INTERRUPTER_TARGET_OFFSET
    }

    pub fn can_in_transfer_ring(&self) -> bool {
        match self.trb_type().unwrap() {
            TrbType::Normal | TrbType::SetupStage | TrbType::DataStage |
                TrbType::StatusStage | TrbType::Isoch | TrbType::Link |
                TrbType:: EventData | TrbType::Noop => true,
            _ => false,
        }
    }

    pub fn transfer_length(&self) -> u32 {
        const STATUS_TRANSFER_LENGTH_MASK: u32 = 0x1ffff;
        match self.trb_type().unwrap() {
            TrbType::Normal | TrbType::SetupStage | TrbType::DataStage | TrbType::Isoch
                => trb.get_status() & STATUS_TRANSFER_LENGTH_MASK,
                _ => 0,
        }
    }

    pub fn interrupt_on_completion(&self) -> bool {
        const FLAGS_INTERRUPT_ON_COMPLETION_MASK: u32= 0x10;
        (self.get_flags() & FLAGS_INTERRUPT_ON_COMPLETION_MASK) > 0
    }

    pub fn immediate_data(&self) -> bool {
        const FLAGS_IMMEDIATE_DATA_MASK: u32 = 0x20;
        match self.trb_type().unwrap() {
            TrbType::Normal | TrbType::SetupStage | TrbType::DataStage | TrbType::Isoch
                -> self.get_flags() & FLAGS_IMMEDIATE_DATA_MASK,
            _ => false,
        }
    }
}

pub trait PrimitiveEnum {
    fn from(val: u8) -> Result<Self>
    where
        Self: std::marker::Sized;
    fn to(&self) -> u8;
}

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
    DisabledOrEnabled = 0,
    Default = 1,
    Addressed = 2,
    Configured = 3,
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

impl SlotContext {
    pub fn state(&self) -> DeviceSlotState {
        DeviceSlotState::from(self.get_slot_state())
    }

    pub fn set_state(&mut self, new_state: DeviceSlotState) {
        self.set_slot_state(state.to());
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

impl EndpointContext {
    pub fn state(&self) -> EndpointState {
        EndpointState::from(self.get_endpoint_state())
    }

    pub fn set_state(&mut self, state: EndpointState) {
        self.set_endpoint_sate(state.to());
    }
}


impl InputControlContext {
    pub fn drop_context_flag(&self, idx: u8) -> bool {
        (self.get_drop_context_flags() &  (1 << idx)) > 0
    }

    pub fn add_context_flag(&self, idx: u8) -> bool {
        (self.get_add_context_flags() &  (1 << idx)) > 0
    }
}


