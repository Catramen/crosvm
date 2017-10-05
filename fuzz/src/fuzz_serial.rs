// Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Fuzzes the serial device

extern crate devices;
extern crate libc;
extern crate sys_util;

use std::os::unix::io::AsRawFd;
use std::slice;

use devices::BusDevice;
use sys_util::EventFd;

fn fuzz_serial(data: &[u8]) {
    let evt = EventFd::new().unwrap();
    unsafe { libc::fcntl(evt.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) };
    let mut serial = devices::Serial::new_sink(evt.try_clone().unwrap());
    for v in data.chunks(2) {
        if v.len() < 2 {
            continue;
        }
        if v[0] & 0x80 != 0 {
            serial.write((v[0] % 8) as u64, &v[1..2]);
        } else {
            let mut out = [0u8; 1];
            serial.read((v[0] % 8) as u64, &mut out[..]);
        }
        if let Err(e) = evt.read() {
            if e.errno() != libc::EAGAIN {
                panic!("unexpected EventFd read error: {}", e.errno());
            }
        }
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
    fuzz_serial(data);
    0
}
