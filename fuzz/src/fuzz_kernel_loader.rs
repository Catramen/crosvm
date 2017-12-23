// Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Fuzzes the kernel loader

extern crate kernel_loader;
extern crate libc;
extern crate sys_util;

use sys_util::{GuestAddress, GuestMemory};

use std::io::Cursor;
use std::slice;


fn fuzz_kernel_loader(data: &[u8]) {
    let mut kimage = Cursor::new(data);
    let mem = GuestMemory::new(&[(GuestAddress(0), data.len() + 0x1000)]).unwrap();
    let result = kernel_loader::load_kernel(&mem, GuestAddress(0), &mut kimage);
    if result.is_err() {
        println!("Not a valid kernel");
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn LLVMFuzzerTestOneInput(
    data: *const libc::uint8_t,
    size: libc::size_t,
) -> libc::c_int {
    let data = unsafe {
        // Safe as long as the caller is trusted not to modify it during this funciton.
        slice::from_raw_parts(data, size)
    };
    fuzz_kernel_loader(data);
    0
}
