// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex};

use usb::xhci::xhci_backend_device::{XhciBackendDevice, UsbDeviceAddress};
use usb::xhci::xhci_transfer::{XhciTransfer, XhciTransferType};
use usb_util::device_handle::DeviceHandle;
use usb_util::usb_transfer::{UsbTransfer, ControlTransferBuffer, control_transfer, TransferStatus};
use usb_util::types::{UsbRequestSetup, ControlRequestDataPhaseTransferDirection, ControlRequestType, ControlRequestRecipient, StandardControlRequest};
use super::usb_endpoint::UsbEndpoint;

#[derive(PartialEq)]
pub enum ControlEndpointState {
    /// Control endpoint has received setup stage.
    SetupStage,
    /// Control endpoint has received data stage.
    DataStage,
    /// Control endpoint has received status stage.
    StatusStage,
}

pub struct HostDevice {
    // Endpoints only contains data endpoints (1 to 30). Control transfers are handled at device
    // level.
    endpoints: Vec<UsbEndpoint>,
    device: LibUsbDevice,
    device_handle: DeviceHandle,
    ctl_ep_state: ControlEndpointState,
    control_transfer: Arc<Mutex<Option<UsbTransfer<ControlTransferBuffer>>>>,
    claimed_interface: Vec<i32>,
    host_claimed_interface: Vec<i32>,
}

impl HostDevice {
    pub fn new(device; LibUsbDevice) -> HostDevice {
        HostDevice {
            endpoints: vec![],
            device: LibUsbDevice,
            device_handle: device.open().unwrap(),
            ctl_ep_state: ControlEndpointState::StatusStage,
            control_transfer: Arc::new(Mutex::new(Some(control_transfer(0)))),
            claimed_interface: vec![],
            host_claimed_interface: vec![],
        }
    }

    fn handle_control_transfer(&mut self, transfer: XhciTransfer) {
        if self.control_transfer.lock().unwrap().is_none() {
            // Yes, we can cache and handle transfers later. But it's easier to not submit transfer
            // before last transfer is handled.
            panic!("Last control transfer has not yet finished.");
        }
        match transfer.get_transfer_type() {
            XhciTransferType::SetupStage(setup) => {
                if self.ctl_ep_state != ControlEndpointState::StatusStage {
                    error!("Control endpoing is in an inconsistant state");
                    return;
                }
                let mut locked = self.control_transfer.lock().unwrap();
                {
                    let request_setup = &mut locked.as_mut().unwrap().mut_buffer().setup_buffer;
                    *request_setup = setup.clone();
                }
                // If the control transfer is device to host, we can go ahead submit the
                // transfer right now.
                if setup.get_direction().unwrap() ==
                    ControlRequestDataPhaseTransferDirection::DeviceToHost {
                        let mut control_transfer = locked.take().unwrap();
                        let myct = self.control_transfer.clone();
                        control_transfer.set_callback(move |t: UsbTransfer<ControlTransferBuffer>| {
                            let status = t.status();
                            let actual_length = t.actual_length();
                            (*myct.lock().unwrap()) = Some(t);
                            transfer.on_transfer_complete(status, actual_length as u32);
                        });
                        self.device_handle.submit_async_transfer(control_transfer);
                } else {
                    transfer.on_transfer_complete(TransferStatus::Completed, 0);
                }
                self.ctl_ep_state = ControlEndpointState::SetupStage;
            },
            XhciTransferType::DataStage(buffer) => {
                 if self.ctl_ep_state != ControlEndpointState::SetupStage {
                    error!("Control endpoing is in an inconsistant state");
                    return;
                 }
                 let mut locked = self.control_transfer.lock().unwrap();
                 let control_transfer = locked.as_mut().unwrap();
                 let tbuffer = control_transfer.mut_buffer();
                 let request_setup = &tbuffer.setup_buffer;
                 match request_setup.get_direction() {
                         Some(ControlRequestDataPhaseTransferDirection::HostToDevice) => {
                             // Read from dma to host buffer.
                             let bytes = buffer.read(&mut tbuffer.data_buffer) as u32;
                             transfer.on_transfer_complete(TransferStatus::Completed, bytes);
                         },
                         Some(ControlRequestDataPhaseTransferDirection::DeviceToHost) => {
                             // For device to host transfer, it's already handled in setup stage.
                             // As ScatterGatherBuffer implementation handles buffer size, we can
                             // copy all buffer.
                             let bytes = buffer.write(&tbuffer.data_buffer) as u32;
                             transfer.on_transfer_complete(TransferStatus::Completed, bytes);
                         },
                         _ => error!("Unknown transfer direction!"),
                 }

                 self.ctl_ep_state = ControlEndpointState::DataStage;

            },
            XhciTransferType::StatusStage => {
                if self.ctl_ep_state == ControlEndpointState::StatusStage {
                    error!("Control endpoing is in an inconsistant state");
                    return;
                }
                let request_setup = {
                    let mut locked = self.control_transfer.lock().unwrap();
                    let control_transfer = locked.as_mut().unwrap();
                    let mut tbuffer = control_transfer.buffer();
                    tbuffer.setup_buffer.clone()
                };
                match request_setup.get_direction() {
                    Some(ControlRequestDataPhaseTransferDirection::HostToDevice) => {
                        // We handle some standard request with libusb api.
                        let s = self.handle_standard_control_requested(&request_setup);
                        if s.is_some() {
                            transfer.on_transfer_complete(s.unwrap(), 0);
                            return;
                        }
                        let mut locked = self.control_transfer.lock().unwrap();
                        let mut control_transfer = locked.take().unwrap();
                        let myct = self.control_transfer.clone();
                        control_transfer.set_callback(move |t: UsbTransfer<ControlTransferBuffer>| {
                            let status = t.status();
                            let actual_length = t.actual_length();
                            (*myct.lock().unwrap()) = Some(t);
                            transfer.on_transfer_complete(status, 0);
                        });
                        self.device_handle.submit_async_transfer(control_transfer);

                    },
                    Some(ControlRequestDataPhaseTransferDirection::DeviceToHost) => {
                        transfer.on_transfer_complete(TransferStatus::Completed, 0);
                    },
                    _ => error!("Unknown transfer direction!"),
                }

                self.ctl_ep_state = ControlEndpointState::StatusStage;
            },
            _ => {
                panic!("Non control transfer sent to control endpoint");
            }
        }
    }

    fn handle_standard_control_requested(&mut self, request_setup: &UsbRequestSetup)
        -> Option<TransferStatus> {
        let s = self.set_address_if_requested(request_setup);
        if s.is_some() {
            return s;
        };
        let s = self.set_config_if_requested(request_setup);
        if s.is_some() {
            return s;
        };
        let s = self.set_interface_if_requested(request_setup);
        if s.is_some() {
            return s;
        };
        let s = self.clear_feature_if_requested(request_setup);
        if s.is_some() {
            return s;
        };
        None
    }

    fn set_address_if_requested(&self, request_setup: &UsbRequestSetup) -> Option<TransferStatus> {
        if request_setup.get_type().unwrap() != ControlRequestType::Standard ||
            request_setup.get_recipient() != ControlRequestRecipient::Device ||
                request_setup.get_standard_request().unwrap() != StandardControlRequest::SetAddress {
                    return None;
        }
        // It's a standard, set_address, device request. We do nothing here.
        debug!("Set address control transfer is received with address: {}", request_setup.value);
        Some(TransferStatus::Completed)
    }

    fn set_config_if_requested(&mut self, request_setup: &UsbRequestSetup) -> Option<TransferStatus> {
        if request_setup.get_type().unwrap() != ControlRequestType::Standard ||
            request_setup.get_recipient() != ControlRequestRecipient::Device ||
                request_setup.get_standard_request().unwrap() != StandardControlRequest::SetConfiguration {
                    return None;
        }
        // It's a standard, set_config, device request.
        let config = (request_setup.value & 0xff) as i32;
        debug!("Set config control transfer is received with config: {}", config);
        self.release_interfaces();
        let cur_config = self.device_handle.get_active_configuration().unwrap();
        debug!("Cur config is: {}", cur_config);
        if config != cur_config {
            self.device_handle.set_active_configuration(config).unwrap();
        }
        self.claim_interfaces();
        self.create_endpoints();
        Some(TransferStatus::Completed)
    }

    fn set_interface_if_requested(&self, request_setup: &UsbRequestSetup) -> Option<TransferStatus> {
        if request_setup.get_type().unwrap() != ControlRequestType::Standard ||
            request_setup.get_recipient() != ControlRequestRecipient::Interface ||
                request_setup.get_standard_request().unwrap() != StandardControlRequest::SetInterface {
                    return None;
        }
        // It's a standard, set_interface, interface request.
        let interface = request_setup.index;
        let alt_setting = request_setup.value;
        self.device_handle.set_interface_alt_setting(interface as i32, alt_setting as i32).unwrap();
        self.create_endpoints();
        Some(TransferStatus::Completed)
    }

    fn clear_feature_if_requested(&self, request_setup: &UsbRequestSetup) -> Option<TransferStatus> {
        if request_setup.get_type().unwrap() != ControlRequestType::Standard ||
            request_setup.get_recipient() != ControlRequestRecipient::Endpoint ||
                request_setup.get_standard_request().unwrap() != StandardControlRequest::ClearFeature {
                    return None;
                }
        // It's a standard, clear_feature, endpoint request.
        const STD_FEATURE_ENDPOINT_HALT: u16 = 0;
        if request_setup.value == STD_FEATURE_ENDPOINT_HALT {
            self.device_handle.clear_halt(request_setup.index as u8);
        }
        Some(TransferStatus::Completed)
    }

    fn release_interfaces(&self) {
        for i in &self.claimed_interface {
            self.device_handle.release_interface(*i).unwrap();
        }

    }

    fn claim_interfaces(&self) {
    }

    fn create_endpoints(&self) {
    }

    fn delete_all_endpoints(&mut self) {
        self.endpoints = vec![];
    }
}

impl XhciBackendDevice for HostDevice {
    fn get_vid(&self) -> u16 {
        self.device.get_device_descriptor.unwrap().idVendor
    }

    fn get_pid(&self) -> u16 {
        self.device.get_device_descriptor.unwrap().idProduct
    }

    fn submit_transfer(&mut self, transfer: XhciTransfer) {
        if transfer.get_endpoint_number() == 0 {
            self.handle_control_transfer(transfer);
            return;
        }
        for ep in &self.endpoints {
            if ep.match_ep(transfer.get_endpoint_number(), &transfer.get_endpoint_dir()) {
                ep.handle_transfer(transfer);
                return;
            }
        }
        warn!("Could not find endpoint for transfer");
        transfer.on_transfer_complete(TransferStatus::Error, 0);
    }

    fn set_address(&mut self, address: UsbDeviceAddress) {
        debug!("Set address is invoked with address: {}", address);
    }
}
