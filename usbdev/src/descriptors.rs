// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use assertions::const_assert;

use data_model::DataInit;
use bit_field::*;

/// Device descriptor size in bytes.
const DEVICE_DESC_SIZE: usize = 18;
/// Config descriptor size in bytes.
const CONFIG_DESC_SIZE: usize = 9;
/// Interface descriptor size in bytes.
const IF_DESC_SIZE: usize = 9;
/// Endpoint descriptor size in bytes.
const EP_DESC_SIZE: usize = 7;
/// Size of descriptor header.
const DESC_HEADER_SIZE: usize = 2;

pub const DEVICE_DESC_TYPE: u8 = 1;
pub const CONFIG_DESC_TYPE: u8 = 2;
pub const IF_DESC_TYPE: u8 = 4;
pub const EP_DESC_TYPE: u8 = 5;

#[bitfield]
#[derive(Copy, Clone, PartialEq)]
pub struct DeviceDescriptor {
    /// Size of this descriptor in bytes.
    length: BitField8,
    /// Descriptor type.
    descriptor_type: BitField8,
    /// USB specification release number in binary-coded decimal. A value of
    /// 0x0200 indicates USB 2.0, 0x0110 indicates USB 1.1, etc.
    bcd_usb: BitField16,
    /// USB-IF class code for the device.
    device_class: BitField8,
    /// USB-IF subclass code for the device.
    device_subclass: BitField8,
    /// USB-IF protocol code for the device.
    device_protocol: BitField8,
    /// Maximum packet size for endpoint 0.
    max_packet_size: BitField8,
    /// USB-IF vendor ID.
    id_vendor: BitField16,
    /// USB-IF product ID.
    id_product: BitField16,
    /// Device release number in binary-coded decimal.
    bcd_device: BitField16,
    /// Index of string descriptor describing manufacturer.
    manufacturer_str_index: BitField8,
    /// Index of string descriptor describing product.
    product_str_index: BitField8,
    /// Index of string descriptor containing device serial number.
    serial_number_str_index: BitField8,
    /// Number of possible configurations.
    num_configs: BitField8,
}

unsafe impl DataInit for DeviceDescriptor {}

#[bitfield]
#[derive(Copy, Clone, PartialEq)]
pub struct ConfigDescriptor {
    /// Size of this descriptor in bytes.
    length: BitField8,
    /// Descriptor type.
    descriptor_type: BitField8,
    /// Total length of data returned for this configuration.
    total_length: BitField16,
    /// Number of interfaces supported by this configuration.
    num_interfaces: BitField8,
    /// Identifier value for this configuration.
    configuration_value: BitField8,
    /// Index of string descriptor describing this configuration.
    configuration_str_index: BitField8,
    /// Configuration characteristics.
    attributes: BitField8,
    /// Maximum power consumption of the USB device from this bus in this
    /// configuration when the device is fully operation. Expressed in units
    /// of 2 mA when the device is operating in high-speed mode and in units
    /// of 8 mA when the device is operating in super-speed mode.
    max_power: BitField8,
}

unsafe impl DataInit for ConfigDescriptor {}

#[bitfield]
#[derive(Copy, Clone, PartialEq)]
pub struct InterfaceDescriptor {
    /// Size of this descriptor in bytes.
    length: BitField8,
    /// Descriptor type.
    descriptor_type: BitField8,
    /// Number of this interface
    interface_number: BitField8,
    /// Value used to select this alternate setting for this interface
    alternate_setting: BitField8,
    /// Number of endpoints used by this interface (excluding the control
    /// endpoint).
    num_endpoints: BitField8,
    /// USB-IF class code for this interface.
    interface_class: BitField8,
    /// USB-IF subclass code for this interface, qualified by the
    /// interface_class value
    interface_subclass: BitField8,
    /// USB-IF protocol code for this interface, qualified by the
    /// interface_class and interface_subClass values
    interface_protocol: BitField8,
    /// Index of string descriptor describing this interface
    interface: BitField8,
}

unsafe impl DataInit for InterfaceDescriptor {}

#[bitfield]
#[derive(Copy, Clone, PartialEq)]
pub struct EndpointDescriptor {
    /// Size of this descriptor (in bytes)
    length: BitField8,
    /// Descriptor type.
    descriptor_type: BitField8,
    /// The address of the endpoint described by this descriptor. Bits 0:3 are
    /// the endpoint number. Bits 4:6 are reserved. Bit 7 indicates direction,
    endpoint_address: BitField8,
    /// Attributes which apply to the endpoint when it is configured using
    /// the configuration_value. Bits 0:1 determine the transfer type. Bits 2:3
    /// are only used for isochronous endpoints. Bits 4:5 are also only used for
    /// isochronous endpoints. Bits 6:7 are reserved.
    attributes: BitField8,
    /// Maximum packet size this endpoint is capable of sending/receiving.
    max_packet_size: BitField16,
    /// Interval for polling endpoint for data transfers.
    interval: BitField8,
}

unsafe impl DataInit for EndpointDescriptor {}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct CommonDescriptorHeader {
    length: u8,
    descriptor_type: u8,
}

unsafe impl DataInit for CommonDescriptorHeader {}

#[derive(PartialEq)]
pub enum DescriptorType {
    Device,
    Config,
    Interface,
    Endpoint,
    Other,
}

impl DescriptorType {
    pub fn new(ty: u8) -> DescriptorType {
        match ty {
            DEVICE_DESC_TYPE => DescriptorType::Device,
            CONFIG_DESC_TYPE => DescriptorType::Config,
            IF_DESC_TYPE => DescriptorType::Interface,
            EP_DESC_TYPE => DescriptorType::Endpoint,
            _ => DescriptorType::Other
        }
    }
}

pub enum Descriptor {
    Device(DeviceDescriptor),
    Config(ConfigDescriptor),
    Interface(InterfaceDescriptor),
    Endpoint(EndpointDescriptor),
    /// Other unparsed descriptor.
    Other(Vec<u8>),
}

pub struct DescriptorIter {
    raw: Vec<u8>,
    // Current parse position.
    position: usize,
}

impl DescriptorIter {
    pub fn new(raw: Vec<u8>) -> DescriptorIter {
        DescriptorIter {
            raw,
            position: 0,
        }
    }

    /// Peek type of the next descriptor.
    pub fn peek_desc_type(&mut self) -> Option<DescriptorType> {
        let header = self.read_header()?;
        Some(DescriptorType::new(header.descriptor_type))
    }

    pub fn read_next_interface_desc_in_this_config(&mut self) -> Option<InterfaceDescriptor> {
        loop {
            // We should not cross config descriptor boundary.
            if self.peek_desc_type()? == DescriptorType::Config {
                return None;
            }
            match self.next()? {
                Descriptor::Interface(if_desc) => return Some(if_desc),
                _ => {},
            }
        }
    }

    pub fn read_next_endpoint_desc_in_this_interface(&mut self) -> Option<EndpointDescriptor> {
        loop {
            match self.peek_desc_type()? {
                DescriptorType::Config | DescriptorType::Interface => return None,
                _ => {
                    match self.next()? {
                        Descriptor::Endpoint(ep_desc) => return Some(ep_desc),
                        _ => {}
                    }
                }
            }
        }
    }

    fn read_header(&mut self) -> Option<CommonDescriptorHeader> {
        if self.position + DESC_HEADER_SIZE > self.raw.len() {
            return None;
        }
        let header = CommonDescriptorHeader::copy_from_slice(
            &self.raw[self.position..(self.position + DESC_HEADER_SIZE)])?;

        if self.position + header.length as usize > self.raw.len() {
            error!("raw descriptor size is not long enough");
            return None;
        }
        Some(header)
    }

    fn read_descriptor<D: DataInit>(&mut self, len_in_header: u8) -> Option<D> {
        let desc_size = std::mem::size_of::<D>();
        // Descriptor might be longer than the bitfields defined in this lib.
        if (len_in_header as usize) < desc_size {
            error!("wrong descriptor size for descriptor");
            return None;
        }
        let desc = D::copy_from_slice(
            &self.raw[self.position..(self.position + desc_size)]
        )?;
        // Trust the length in header.
        self.position += len_in_header as usize;
        Some(desc)
    }
}

impl Iterator for DescriptorIter {
    type Item = Descriptor;

    fn next(&mut self) -> Option<Descriptor> {
        let header = self.read_header()?;
        match header.descriptor_type {
            DEVICE_DESC_TYPE => {
                let desc: DeviceDescriptor = self.read_descriptor(header.length)?;
                Some(Descriptor::Device(desc))
            },
            CONFIG_DESC_TYPE => {
                let desc: ConfigDescriptor = self.read_descriptor(header.length)?;
                Some(Descriptor::Config(desc))
            },
            IF_DESC_TYPE => {
                let desc: InterfaceDescriptor = self.read_descriptor(header.length)?;
                Some(Descriptor::Interface(desc))
            },
            EP_DESC_TYPE => {
                let desc: EndpointDescriptor = self.read_descriptor(header.length)?;
                Some(Descriptor::Endpoint(desc))
            },
            _ => {
                let mut desc: Vec<u8> = vec![];
                desc.extend_from_slice(&self.raw[self.position..(self.position + header.length as usize)]);
                self.position += header.length as usize;
                Some(Descriptor::Other(desc))
            }
        }
    }
}


fn _assert() {
    const_assert!(std::mem::size_of::<DeviceDescriptor>() == DEVICE_DESC_SIZE);
    const_assert!(std::mem::size_of::<ConfigDescriptor>() == CONFIG_DESC_SIZE);
    const_assert!(std::mem::size_of::<InterfaceDescriptor>() == IF_DESC_SIZE);
    const_assert!(std::mem::size_of::<EndpointDescriptor>() == EP_DESC_SIZE);
    const_assert!(std::mem::size_of::<CommonDescriptorHeader>() == DESC_HEADER_SIZE);
}
