// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#[derive(Debug)]
pub enum Error {
    // Unable to access sysfs folders.
    UnableToAccess,
    // There is no such device.
    NoDevice,
    // Cannot perform IO.
    IO,
    // Unexpected error.
    Other
}

pub type Result<T> = std::result::Result<T, Error>;
