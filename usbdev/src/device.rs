// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::path::{Path, PathBuf};
use std::fs::{self, File};

use error::*;
use descriptors::*;

const SYSFS_DEVICES_PATH: &str = "/sys/bus/usb/devices";

#[derive(Debug, Clone)]
pub struct Config {
    pub desc: ConfigDescriptor,
    pub interfaces: Vec<InterfaceAltSettings>
}

impl Config {
    pub fn get_interface(&self, if_num: u8, alt_setting: u8 ) -> Option<&Interface> {
        for ias in &self.interfaces {
            for i in &ias.alt_settings {
                if i.desc.get_interface_number() == if_num && i.desc.get_alternate_setting() == alt_setting {
                    return Some(i);
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct InterfaceAltSettings {
    pub alt_settings: Vec<Interface>,
}

#[derive(Debug, Clone)]
pub struct Interface {
    pub desc: InterfaceDescriptor,
    pub endpoints: Vec<EndpointDescriptor>,
}

impl Interface {
    fn read_from(iter: &mut DescriptorIter) -> Option<Interface> {
        let interface_desc = iter.read_next_interface_desc_in_this_config()?;

        // Read all endpoint descriptors of this interface.
        let mut endpoints = vec![];
        for _ in 0..interface_desc.get_num_endpoints() {
            let endpoint_desc =  iter.read_next_endpoint_desc_in_this_interface()?;
            endpoints.push(endpoint_desc);
        }
        Some(Interface {
            desc: interface_desc,
            endpoints,
        })
    }
}

#[derive(Debug)]
enum State {
    // We got information of this device.
    Info,
    // We have opened the device.
    Opened(File),
    // We think the device is failed.
    Failed,
    // We think the device is already unplugged.
    Unplugged,
}

#[derive(Debug)]
pub struct Device {
    busnum: u8,
    devnum: u8,
    device_desc: DeviceDescriptor,
    configs: Vec<Config>,
    // Path to the sysfs folder of this device.
    sysfs_dir: String,
    state: State,
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

    pub fn set_unplug_callback() {
    }

    pub fn open(&mut self, fd: File) {
        self.state = State::Opened(fd);
    }

    pub fn get_busnum(&self) -> u8 {
        self.busnum
    }

    pub fn get_devnum(&self) -> u8 {
        self.devnum
    }

    pub fn get_device_descriptor(&self) -> &DeviceDescriptor {
        &self.device_desc
    }

    pub fn get_configs(&self) -> &[Config] {
        self.configs.as_slice()
    }

    pub fn get_config_by_value(&self, cfg_val: u8) -> Option<&Config> {
        for c in &self.configs {
            if c.desc.get_configuration_value() == cfg_val {
                return Some(&c);
            }
        }
        None
    }

    pub fn get_active_config_value(&self) -> Result<u8> {
        Self::read_and_parse(&self.sysfs_dir, "bConfigurationValue").ok_or(Error::IO)
    }

    pub fn get_active_config(&self) -> Result<&Config> {
        let cfg_val = self.get_active_config_value()?;
        self.get_config_by_value(cfg_val)
            .ok_or_else(|| {
                error!("cannot find config descriptor for current active config {}", cfg_val);
                Error::Other
            })
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
            state: State::Info
        })
    }

    fn read_and_parse<T: std::str::FromStr, P: AsRef<Path>>(path: &P, file_name: &str) -> Option<T> {
        let file_path = path.as_ref().join(file_name);
        let val = fs::read_to_string(file_path).ok()?.trim().parse().ok()?;
        Some(val)
    }

    fn read_busnum(path: &PathBuf) -> Option<u8> {
        Self::read_and_parse(path, "busnum")
    }

    fn read_devnum(path: &PathBuf) -> Option<u8> {
        Self::read_and_parse(path, "devnum")
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

        // The following outer loop will grap the next config descriptor, skip unexpected ones.
        // Thus inner loop is bounded by config descriptors.
        let mut configs: Vec<Config> = vec![];
        for _ in 0..device_desc.get_num_configs() {
            let config_desc = match iter.next()? {
                Descriptor::Config(d) => d,
                _ => continue,
            };
            let mut interfaces: Vec<InterfaceAltSettings> = vec![];

            // The following loop group interface_descriptors into alt_settings by interface_num.
            let mut cur_interface_num: i16 = -1;
            let mut alt_settings = vec![];
            loop {
                // Try to read next alt_settings.
                let interface = match Interface::read_from(&mut iter) {
                    Some(a) => a,
                    None => {
                        // There is no more alt settings, push the last one.
                        interfaces.push(InterfaceAltSettings {
                            alt_settings,
                        });
                        break;
                    }
                };

                // Init cur_interface_num when we meet the first interface descriptor.
                if cur_interface_num == -1 {
                    cur_interface_num = interface.desc.get_interface_number() as i16;
                }

                // If it is the same interface_num, it's in the same alt_settings set.
                if cur_interface_num == interface.desc.get_interface_number() as i16 {
                    alt_settings.push(interface);
                } else {
                    // If it is a new interface_num, we creat a new set of alt_settings and push
                    // the older one into interfaces.
                    cur_interface_num = interface.desc.get_interface_number() as i16;
                    let mut tmp = vec![];
                    std::mem::swap(&mut tmp, &mut alt_settings);
                    alt_settings.push(interface);

                    interfaces.push(InterfaceAltSettings {
                        alt_settings: tmp
                    });
                }
            }

            configs.push(Config {
                desc: config_desc,
                interfaces,
            });
        }
        Some((device_desc, configs))
    }
}



