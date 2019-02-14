// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#[derive(Debug)]
pub enum Error {
    UnableToAccess,
    NoSuchDevice,
}

pub type Result<T> = std::result::Result<T, Error>;
