// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use data_model::DataInit;

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

const DEVICE_DESC_TYPE: u8 = 1;
const CONFIG_DESC_TYPE: u8 = 2;
const IF_DESC_TYPE: u8 = 4;
const EP_DESC_TYPE: u8 = 5;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct DeviceDescriptor {
    /// Size of this descriptor in bytes.
    pub length: u8,
    /// Descriptor type.
    pub descriptor_type: u8,
    /// USB specification release number in binary-coded decimal. A value of
    /// 0x0200 indicates USB 2.0, 0x0110 indicates USB 1.1, etc.
    pub bcd_usb: u16,
    /// USB-IF class code for the device.
    pub device_class: u8,
    /// USB-IF subclass code for the device.
    pub device_subclass: u8,
    /// USB-IF protocol code for the device.
    pub device_protocol: u8,
    /// Maximum packet size for endpoint 0.
    pub max_packet_size: u8,
    /// USB-IF vendor ID.
    pub id_vendor: u16,
    /// USB-IF product ID.
    pub id_product: u16,
    /// Device release number in binary-coded decimal.
    pub bcd_device: u16,
    /// Index of string descriptor describing manufacturer.
    pub manufacturer_str_index: u8,
    /// Index of string descriptor describing product.
    pub product_str_index: u8,
    /// Index of string descriptor containing device serial number.
    pub serial_number_str_index: u8,
    /// Number of possible configurations.
    pub num_configs: u8,
}

unsafe impl DataInit for DeviceDescriptor {}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ConfigDescriptor {
    /// Size of this descriptor in bytes.
    pub length: u8,
    /// Descriptor type.
    pub descriptor_type: u8,
    /// Total length of data returned for this configuration.
    pub total_length: u16,
    /// Number of interfaces supported by this configuration.
    pub num_interfaces: u8,
    /// Identifier value for this configuration.
    pub configuration_value: u8,
    /// Index of string descriptor describing this configuration.
    pub configuration_str_index: u8,
    /// Configuration characteristics.
    pub attributes: u8,
    /// Maximum power consumption of the USB device from this bus in this
    /// configuration when the device is fully operation. Expressed in units
    /// of 2 mA when the device is operating in high-speed mode and in units
    /// of 8 mA when the device is operating in super-speed mode.
    pub max_power: u8,
}

unsafe impl DataInit for ConfigDescriptor {}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct InterfaceDescriptor {
    /// Size of this descriptor in bytes.
    pub length: u8,
    /// Descriptor type.
    pub descriptor_type: u8,
    /// Number of this interface
    pub interface_number: u8,
    /// Value used to select this alternate setting for this interface
    pub alternate_setting: u8,
    /// Number of endpoints used by this interface (excluding the control
    /// endpoint).
    pub num_endpoints: u8,
    /// USB-IF class code for this interface.
    pub interface_class: u8,
    /// USB-IF subclass code for this interface, qualified by the
    /// interface_class value
    pub interface_subclass: u8,
    /// USB-IF protocol code for this interface, qualified by the
    /// interface_class and interface_subClass values
    pub interface_protocol: u8,
    /// Index of string descriptor describing this interface
    pub interface: u8,
}

unsafe impl DataInit for InterfaceDescriptor {}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct EndpointDescriptor {
    /// Size of this descriptor (in bytes)
    pub length: u8,
    /// Descriptor type.
    pub descriptor_type: u8,
    /// The address of the endpoint described by this descriptor. Bits 0:3 are
    /// the endpoint number. Bits 4:6 are reserved. Bit 7 indicates direction,
    pub endpoint_address: u8,
    /// Attributes which apply to the endpoint when it is configured using
    /// the configuration_value. Bits 0:1 determine the transfer type. Bits 2:3
    /// are only used for isochronous endpoints. Bits 4:5 are also only used for
    /// isochronous endpoints. Bits 6:7 are reserved.
    pub attributes: u8,
    /// Maximum packet size this endpoint is capable of sending/receiving.
    pub max_packet_size: u16,
    /// Interval for polling endpoint for data transfers.
    pub interval: u8,
    /// For audio devices only: the rate at which synchronization feedback
    /// is provided.
    pub refresh: u8,
    /// For audio devices only: the address if the synch endpoint
    pub synch_address: u8,
}

unsafe impl DataInit for EndpointDescriptor {}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct CommonDescriptorHeader {
    length: u8,
    descriptor_type: u8,
}

unsafe impl DataInit for CommonDescriptorHeader {}

pub enum Descriptor {
    Device(DeviceDescriptor),
    Config(ConfigDescriptor),
    Interface(InterfaceDescriptor),
    Endpoint(EndpointDescriptor),
    /// Other unparsed descriptor.
    Other(Vec<u8>),
}

impl Descriptor {
    pub fn parse(raw: Vec<u8>) -> DescriptorIter {
        DescriptorIter {
            raw,
            position: 0,
        }
    }
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

    fn read_descriptor<D: DataInit>(&mut self, len_in_header: u8) -> Option<D> {
        let desc_size = std::mem::size_of::<D>();
        if len_in_header as usize != desc_size {
            error!("wrong descriptor size for descriptor");
            return None;
        }
        let desc = D::copy_from_slice(
            &self.raw[self.position..(self.position + desc_size)]
        )?;
        self.position += desc_size;
        Some(desc)
    }
}

impl Iterator for DescriptorIter {
    type Item = Descriptor;

    fn next(&mut self) -> Option<Descriptor> {
        if self.position + DESC_HEADER_SIZE > self.raw.len() {
            return None;
        }
        let header = CommonDescriptorHeader::copy_from_slice(
            &self.raw[self.position..(self.position + DESC_HEADER_SIZE)])?;

        if self.position + header.length as usize > self.raw.len() {
            error!("raw descriptor size is not long enough");
            return None;
        }

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

mod tests {
    #[test]
    fn descriptor_sizes() {
        assert_eq!(std::mem::size_of::<DeviceDescriptor>(), DEVICE_DESC_SIZE);
        assert_eq!(std::mem::size_of::<ConfigDescriptor>(), CONFIG_DESC_SIZE);
        assert_eq!(std::mem::size_of::<InterfaceDescriptor>(), IF_DESC_SIZE);
        assert_eq!(std::mem::size_of::<EndpointDescriptor>(), EP_DESC_SIZE);
        assert_eq!(std::mem::size_of::<CommonDescriptorHeader>(), DESC_HEADER_SIZE);
    }
}
