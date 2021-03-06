// Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Implements virtio devices, queues, and transport mechanisms.

mod balloon;
mod queue;
mod mmio;
mod block;
mod rng;
mod net;
mod wl;

pub mod vhost;

pub use self::balloon::*;
pub use self::queue::*;
pub use self::mmio::*;
pub use self::block::*;
pub use self::rng::*;
pub use self::net::*;
pub use self::wl::*;

const DEVICE_ACKNOWLEDGE: u32 = 0x01;
const DEVICE_DRIVER: u32 = 0x02;
const DEVICE_DRIVER_OK: u32 = 0x04;
const DEVICE_FEATURES_OK: u32 = 0x08;
const DEVICE_FAILED: u32 = 0x80;

// Types taken from linux/virtio_ids.h
const TYPE_NET: u32 = 1;
const TYPE_BLOCK: u32 = 2;
const TYPE_RNG: u32 = 4;
const TYPE_BALLOON: u32 = 5;
const TYPE_VSOCK: u32 = 19;
const TYPE_WL: u32 = 30;

const INTERRUPT_STATUS_USED_RING: u32 = 0x1;
const INTERRUPT_STATUS_CONFIG_CHANGED: u32 = 0x2;

/// Offset from the base MMIO address of a virtio device used by the guest to notify the device of
/// queue events.
pub const NOTIFY_REG_OFFSET: u32 = 0x50;
