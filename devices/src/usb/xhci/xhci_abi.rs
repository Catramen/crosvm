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

#[derive(BitField)]
#[passthrough(derive(Clone, Copy))]

