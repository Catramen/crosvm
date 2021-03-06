// Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Emulates virtual and hardware devices.

extern crate byteorder;
extern crate data_model;
extern crate io_jail;
extern crate libc;
extern crate net_sys;
extern crate net_util;
#[macro_use]
extern crate sys_util;
extern crate vhost;
extern crate virtio_sys;
extern crate vm_control;

mod bus;
mod cmos;
mod i8042;
mod proxy;
mod serial;
pub mod pl030;
pub mod virtio;
pub mod usb;

pub use self::bus::{Bus, BusDevice};
pub use self::cmos::Cmos;
pub use self::pl030::Pl030;
pub use self::i8042::I8042Device;
pub use self::proxy::ProxyDevice;
pub use self::proxy::Error as ProxyError;
pub use self::serial::Serial;
