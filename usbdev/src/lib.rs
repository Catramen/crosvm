// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
// Generated with bindgen usbdevice_fs.h -no-prepend-enum-name -o bindings.rs.

#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#[cfg_attr(feature = "cargo-clippy", allow(clippy))]
mod bindings;

extern crate assertions;
extern crate data_model;
extern crate sync;
#[macro_use]
extern crate sys_util;
#[macro_use]
extern crate bit_field;

#[macro_use]
mod error;
mod descriptors;
mod device;
mod ioctl;

pub use error::*;
pub use descriptors::*;
pub use device::*;
