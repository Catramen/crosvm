// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::path::{Path, PathBuf};
use std::fs::{self, File};

use error::*;

const SYSFS_DEVICES_PATH: &str = "/sys/bus/usb/devices";

// A usb device.
pub struct Device {
    // Path to the sysfs folder of this device.
    sysfs_dir: Path,
    // An open file of the usbfs node.
    usbfs_file: File,
}

impl Device {
    pub fn new(bus: u8,
               addr: u8,
               vid: u16,
               pid: u16,
               usbfs_file: File) -> Result<Device> {
        let sysfs_path = Path::new(SYSFS_DEVICES_PATH);
        if !sysfs_path.is_dir() {
            error!("cannot open sysfs folder {}", SYSFS_DEVICES_PATH);
            return Err(Error::NoSuchDevice);
        }

        for entry in fs::read_dir(dir).map_err(Error::NoSuchDevice)? {
            let entry = entry.map_err(Error::NoSuchDevice)?;
            let path = entry.path();
            if path.is_dir() {

            }
        }
    }

    pub fn set_unplug_callback() {
    }

    fn match_device(bus:u8, addr: u8, vid: u16, pid: u16, path: &PathBuf) -> Option<Path> {
        if bus != read_bus(path)? {
            return None;
        }
        if addr != read_addr(path)? {
            return None;
        }
        if vid != read_vid(path)? {
            return None;
        }
        if pid != read_pid(path)? {
            return None;
        }
        Some(path.as_path().clone())
    }

    fn read_bus(path: &PathBuf) -> Option<u8> {
    }

    fn read_addr(path: &PathBuf) -> Option<u8> {
    }

    fn read_vid(path: &PathBuf) -> Option<u16> {
    }

    fn read_pid(path: &PathBuf) -> Option<u16> {
    }
}



