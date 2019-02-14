// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::path::{Path, PathBuf};
use std::fs::{self, File};

use error::*;
use descriptors::*;

const SYSFS_DEVICES_PATH: &str = "/sys/bus/usb/devices";

#[derive(Debug)]
pub struct Config {
    pub desc: ConfigDescriptor,
    pub interfaces: Vec<Interface>
}

#[derive(Debug)]
pub struct Interface {
    pub desc: InterfaceDescriptor,
    pub endpoints: Vec<EndpointDescriptor>,
}

#[derive(Debug)]
pub struct Device {
    busnum: u8,
    devnum: u8,
    device_desc: DeviceDescriptor,
    configs: Vec<Config>,
    // Path to the sysfs folder of this device.
    sysfs_dir: String,
}

impl Device {
    pub fn device_list() -> Result<Vec<Device>> {
        let sysfs_path = Path::new(SYSFS_DEVICES_PATH);
        if !sysfs_path.is_dir() {
            error!("cannot open sysfs folder {}", SYSFS_DEVICES_PATH);
            return Err(Error::UnableToAccess);
        }

        let mut devices = vec![];
        for entry in fs::read_dir(sysfs_path).map_err(|_| Error::UnableToAccess)? {
            let entry = entry.map_err(|_| Error::UnableToAccess)?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(d) = Device::new(&path) {
                    devices.push(d);
                }
            }
        }
        Ok(devices)
    }

    fn new(path: &PathBuf) -> Option<Device> {
        let busnum = Self::read_busnum(path)?;
        let devnum = Self::read_devnum(path)?;
        let (device_desc, configs) = Self::read_descriptors(path)?;
        Some(Device {
            busnum,
            devnum,
            device_desc,
            configs,
            sysfs_dir: String::from(path.to_str()?),
        })
    }

    fn read_busnum(path: &PathBuf) -> Option<u8> {
        let mut busnum_path = path.clone();
        busnum_path.push("busnum");
        let busnum: u8 = fs::read_to_string(busnum_path).ok()?.parse().ok()?;
        Some(busnum)
    }

    fn read_devnum(path: &PathBuf) -> Option<u8> {
        let mut devnum_path = path.clone();
        devnum_path.push("devnum");
        let devnum: u8 = fs::read_to_string(devnum_path).ok()?.parse().ok()?;
        Some(devnum)
    }

    fn read_descriptors(path: &PathBuf) -> Option<(DeviceDescriptor, Vec<Config>)> {
        let mut desc_path = path.clone();
        desc_path.push("descriptors");
        let raw_desc = fs::read(desc_path).ok()?;
        let mut iter = DescriptorIter::new(raw_desc);
        // First descriptor is device descriptor.
        let device_desc = match iter.next()? {
            Descriptor::Device(d) => d,
            _ => {
                error!("cannot parse device desc");
                return None;
            }
        };

        // The following nested loop will grap the next expected descriptor, skip unexpected ones.
        let mut configs: Vec<Config> = vec![];
        for _ in 0..device_desc.num_configs {
            let config_desc = match iter.next()? {
                Descriptor::Config(d) => d,
                _ => continue,
            };
            let mut interfaces: Vec<Interface> = vec![];
            for _ in 0..config_desc.num_interfaces {
                let interface_desc = match iter.next()? {
                    Descriptor::Interface(d) => d,
                    _ => continue,
                };

                let mut endpoints = vec![];
                for _ in 0..interface_desc.num_endpoints {
                    let endpoint_desc = match iter.next()? {
                        Descriptor::Endpoint(d) => d,
                        _ => continue,
                    };
                    endpoints.push(endpoint_desc);
                }
                interfaces.push(Interface {
                    desc: interface_desc,
                    endpoints,
                });
            }
            configs.push(Config {
                desc: config_desc,
                interfaces
            });
        }
        Some((device_desc, configs))
    }

    pub fn set_unplug_callback() {
    }

    pub fn open(&self, _: File) {
    }
}



