// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex};

use usb::xhci::xhci_backend_device::{XhciBackendDevice, UsbDeviceAddress};
use usb::xhci::xhci_transfer::{XhciTransfer, XhciTransferType};
use usb_util::libusb_device::LibUsbDevice;
use usb_util::device_handle::DeviceHandle;
use usb_util::usb_transfer::{UsbTransfer, ControlTransferBuffer, control_transfer, TransferStatus};
use usb_util::types::{UsbRequestSetup, ControlRequestDataPhaseTransferDirection, ControlRequestType, ControlRequestRecipient, StandardControlRequest};
use usb_util::error::Error as LibUsbError;
use std::collections::HashMap;
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

// Types of host to device control requests. We want to handle it use libusb functions instead of
// control transfers.
enum HostToDeviceControlRequest {
    SetAddress,
    SetConfig,
    SetInterface,
    ClearFeature,
    // It could still be some standard control request.
    Other,
}

impl HostToDeviceControlRequest {
    pub fn analyze_request_setup(request_setup: &UsbRequestSetup)
        -> HostToDeviceControlRequest {
            if request_setup.get_type().unwrap() == ControlRequestType::Standard &&
                request_setup.get_recipient() == ControlRequestRecipient::Device &&
                    request_setup.get_standard_request() ==
                    Some(StandardControlRequest::SetAddress) {
                        return HostToDeviceControlRequest::SetAddress;
                    };
            if request_setup.get_type().unwrap() == ControlRequestType::Standard &&
                request_setup.get_recipient() == ControlRequestRecipient::Device &&
                    request_setup.get_standard_request() ==
                    Some(StandardControlRequest::SetConfiguration) {
                        return HostToDeviceControlRequest::SetConfig;
                    };
            if request_setup.get_type().unwrap() == ControlRequestType::Standard &&
                request_setup.get_recipient() == ControlRequestRecipient::Interface &&
                    request_setup.get_standard_request() ==
                    Some(StandardControlRequest::SetInterface) {
                        return HostToDeviceControlRequest::SetInterface;
                    };
            if request_setup.get_type().unwrap() == ControlRequestType::Standard &&
                request_setup.get_recipient() == ControlRequestRecipient::Endpoint &&
                    request_setup.get_standard_request() ==
                    Some(StandardControlRequest::ClearFeature) {
                        return HostToDeviceControlRequest::ClearFeature;
                    };
            return HostToDeviceControlRequest::Other;
    }
}

pub struct HostDevice {
    // Endpoints only contains data endpoints (1 to 30). Control transfers are handled at device
    // level.
    endpoints: Vec<UsbEndpoint>,
    device: LibUsbDevice,
    device_handle: Arc<Mutex<DeviceHandle>>,
    ctl_ep_state: ControlEndpointState,
    control_transfer: Arc<Mutex<Option<UsbTransfer<ControlTransferBuffer>>>>,
    alt_settings: HashMap<u16, u16>,
    claimed_interfaces: Vec<i32>,
    host_claimed_interfaces: Vec<i32>,
}

impl Drop for HostDevice {
    fn drop(&mut self) {
        self.release_interfaces();
        self.attach_host_drivers();
    }
}

impl HostDevice {
    pub fn new(device: LibUsbDevice) -> HostDevice {
        let device_handle = Arc::new(Mutex::new(device.open().unwrap()));
        let mut device = HostDevice {
            endpoints: vec![],
            device,
            device_handle,
            ctl_ep_state: ControlEndpointState::StatusStage,
            control_transfer: Arc::new(Mutex::new(Some(control_transfer(0)))),
            alt_settings: HashMap::new(),
            claimed_interfaces: vec![],
            host_claimed_interfaces: vec![],
        };
        device.detach_host_drivers();
        device
    }

    fn get_interface_number_of_active_config(&self) -> i32 {
        match self.device.get_active_config_descriptor() {
            Err(LibUsbError::NotFound) => {
                debug!("device is in unconfigured state");
                0
            },
            Err(e) => {
                // device might be disconnected now.
                error!("unexpected error {:?}", e);
                0
            },
            Ok(descriptor) => descriptor.bNumInterfaces as i32,
        }
    }
    fn detach_host_drivers(&mut self) {
        for i in 0..self.get_interface_number_of_active_config() {
            match self.device_handle.lock().unwrap().kernel_driver_active(i) {
                Ok(true) => {
                    if let Err(e) =
                        self.device_handle.lock().unwrap().detach_kernel_driver(i as i32) {
                        error!("unexpectd error {:?}", e);
                    } else {
                        debug!("host driver detached for interface {}", i);
                        self.host_claimed_interfaces.push(i);
                    }

                },
                Ok(false) => {
                    debug!("no driver attached");
                },
                Err(e) => {
                    error!("unexpected error {:?}", e);
                }
            }
        }
    }

    fn release_interfaces(&mut self) {
        for i in &self.claimed_interfaces {
            if let Err(e) = self.device_handle.lock().unwrap().release_interface(*i) {
                error!("could not release interface {:?}", e);
            }
        }
        self.claimed_interfaces = Vec::new();
    }

    fn attach_host_drivers(&mut self) {
        for i in &self.host_claimed_interfaces {
            if let Err(e) = self.device_handle.lock().unwrap().attach_kernel_driver(*i) {
                error!("could not attach host kernel {:?}", e);
            }
        }
    }

    fn handle_control_transfer(&mut self, transfer: XhciTransfer) {
        if self.control_transfer.lock().unwrap().is_none() {
            // Yes, we can cache and handle transfers later. But it's easier to not submit transfer
            // before last transfer is handled.
            panic!("Last control transfer has not yet finished.");
        }
        let xhci_transfer = Arc::new(transfer);
        match xhci_transfer.get_transfer_type() {
            XhciTransferType::SetupStage(setup) => {
                if self.ctl_ep_state != ControlEndpointState::StatusStage {
                    error!("Control endpoing is in an inconsistant state");
                    return;
                }
                let mut locked = self.control_transfer.lock().unwrap();
                // Copy request setup into control transfer buffer.
                locked.as_mut().unwrap().mut_buffer().set_request_setup(&setup);

                // If the control transfer is device to host, we can go ahead submit the
                // transfer right now.
                if setup.get_direction().unwrap() ==
                    ControlRequestDataPhaseTransferDirection::DeviceToHost {
                        // Control transfer works like yoyo. It submited to device and will be put
                        // back when callback is done.
                        let mut control_transfer = locked.take().unwrap();
                        let weak_control_transfer = Arc::downgrade(&self.control_transfer);
                        let tmp_transfer = xhci_transfer.clone();
                        control_transfer.set_callback(move |t: UsbTransfer<ControlTransferBuffer>| {
                            let status = t.status();
                            let actual_length = t.actual_length();
                            if let Some(control_transfer) = weak_control_transfer.upgrade() {
                                (*control_transfer.lock().unwrap()) = Some(t);
                            }
                            xhci_transfer.on_transfer_complete(status, actual_length as u32);
                        });
                        match self.device_handle.lock().unwrap().submit_async_transfer(control_transfer) {
                            Err((e, t)) => {
                                error!("fail to submit control transfer {:?}", e);
                                tmp_transfer.on_transfer_complete(TransferStatus::Error, 0);
                                // Put the transfer back.
                                *locked = Some(t);
                            },
                            _ => {},
                        }
                } else {
                    xhci_transfer.on_transfer_complete(TransferStatus::Completed, 0);
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
                             xhci_transfer.on_transfer_complete(TransferStatus::Completed, bytes);
                         },
                         Some(ControlRequestDataPhaseTransferDirection::DeviceToHost) => {
                             // For device to host transfer, it's already handled in setup stage.
                             // As ScatterGatherBuffer implementation handles buffer size, we can
                             // copy all buffer.
                             let bytes = buffer.write(&tbuffer.data_buffer) as u32;
                             xhci_transfer.on_transfer_complete(TransferStatus::Completed, bytes);
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
                        match HostToDeviceControlRequest::analyze_request_setup(&request_setup) {
                            HostToDeviceControlRequest::Other => {
                                let mut locked = self.control_transfer.lock().unwrap();
                                let mut control_transfer = locked.take().unwrap();
                                let myct = self.control_transfer.clone();
                                let tmp_transfer = xhci_transfer.clone();
                                control_transfer.set_callback(
                                    move |t: UsbTransfer<ControlTransferBuffer>| {
                                        let status = t.status();
                                        // Use actual length soon.
                                        let _actual_length = t.actual_length();
                                        (*myct.lock().unwrap()) = Some(t);
                                        xhci_transfer.on_transfer_complete(status, 0);
                                    });
                                if let Err((e, t)) =
                                    self.device_handle.lock().unwrap().submit_async_transfer(control_transfer) {
                                        error!("fail to submit control transfer {:?}", e);
                                        tmp_transfer.on_transfer_complete(TransferStatus::Error, 0);
                                        *locked = Some(t);
                                    };
                            },
                            HostToDeviceControlRequest::SetAddress => {
                                let status = self.set_address(request_setup.value as u32);
                                xhci_transfer.on_transfer_complete(TransferStatus::Completed, 0);
                            },
                            HostToDeviceControlRequest::SetConfig => {
                                let status = self.set_config(&request_setup);
                                xhci_transfer.on_transfer_complete(status, 0);
                            },
                            HostToDeviceControlRequest::SetInterface => {
                                let status = self.set_interface(&request_setup);
                                xhci_transfer.on_transfer_complete(status, 0);
                            },
                            HostToDeviceControlRequest::ClearFeature => {
                                let status = self.clear_feature(&request_setup);
                                xhci_transfer.on_transfer_complete(status, 0);
                            }
                        };
                    },
                    Some(ControlRequestDataPhaseTransferDirection::DeviceToHost) => {
                        xhci_transfer.on_transfer_complete(TransferStatus::Completed, 0);
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

    fn set_config(&mut self, request_setup: &UsbRequestSetup) -> TransferStatus {
        // It's a standard, set_config, device request.
        let config = (request_setup.value & 0xff) as i32;
        debug!("Set config control transfer is received with config: {}", config);
        self.release_interfaces();
        let cur_config = self.device_handle.lock().unwrap().get_active_configuration().unwrap();
        debug!("Cur config is: {}", cur_config);
        if config != cur_config {
            self.device_handle.lock().unwrap().set_active_configuration(config).unwrap();
        }
        self.claim_interfaces();
        self.create_endpoints();
        TransferStatus::Completed
    }

    fn set_interface(&mut self, request_setup: &UsbRequestSetup) -> TransferStatus {
        // It's a standard, set_interface, interface request.
        let interface = request_setup.index;
        let alt_setting = request_setup.value;
        self.device_handle.lock().unwrap()
            .set_interface_alt_setting(interface as i32, alt_setting as i32).unwrap();
        self.alt_settings.insert(interface, alt_setting);
        self.create_endpoints();
        TransferStatus::Completed
    }

    fn clear_feature(&mut self, request_setup: &UsbRequestSetup) -> TransferStatus {
        // It's a standard, clear_feature, endpoint request.
        const STD_FEATURE_ENDPOINT_HALT: u16 = 0;
        if request_setup.value == STD_FEATURE_ENDPOINT_HALT {
            self.device_handle.lock().unwrap().clear_halt(request_setup.index as u8).unwrap();
        }
        TransferStatus::Completed
    }

    fn claim_interfaces(&mut self) {
        for i in 0..self.get_interface_number_of_active_config() {
            match self.device_handle.lock().unwrap().claim_interface(i) {
                Ok(()) => {
                    self.claimed_interfaces.push(i);
                },
                Err(e) => {
                    error!("unable to claim interface");
                }
            }
        }
    }

    fn create_endpoints(&mut self) {
        self.endpoints = Vec::new();
        let config_descriptor = match self.device.get_active_config_descriptor() {
            Err(e) => {
                // device might be disconnected now.
                error!("unexpected error {:?}", e);
                return;
            },
            Ok(descriptor) => descriptor,
        };
        for i in &self.claimed_interfaces {
            let alt_setting = self.alt_settings.get(&(*i as u16)).unwrap_or(&0);
            let interface = config_descriptor.
                get_interface_descriptor(*i as u8, *alt_setting as i32).unwrap();
            for ep_idx in 0..interface.bNumEndpoints {
                let ep_dp = interface.endpoint_descriptor(ep_idx).unwrap();
                let ep_num = ep_dp.get_endpoint_number();
                if ep_num == 0 {
                    debug!("endpoint 0 in endpoint descriptors");
                    continue;
                }
                let direction = ep_dp.get_direction();
                let ty = ep_dp.get_endpoint_type().unwrap();
                self.endpoints.push(
                    UsbEndpoint::new(self.device_handle.clone(), ep_num, direction, ty));
            }
        }
    }

    fn delete_all_endpoints(&mut self) {
        self.endpoints = vec![];
    }
}

impl XhciBackendDevice for HostDevice {
    fn get_vid(&self) -> u16 {
        self.device.get_device_descriptor().unwrap().idVendor
    }

    fn get_pid(&self) -> u16 {
        self.device.get_device_descriptor().unwrap().idProduct
    }

    fn submit_transfer(&mut self, transfer: XhciTransfer) {
        if transfer.get_endpoint_number() == 0 {
            self.handle_control_transfer(transfer);
            return;
        }
        for ep in &self.endpoints {
            if ep.match_ep(transfer.get_endpoint_number(), &transfer.get_transfer_dir()) {
                ep.handle_transfer(transfer);
                return;
            }
        }
        warn!("Could not find endpoint for transfer");
        transfer.on_transfer_complete(TransferStatus::Error, 0);
    }

    fn set_address(&mut self, address: UsbDeviceAddress) {
        // It's a standard, set_address, device request. We do nothing here. As descripted in XHCI
        // spec. See set address command ring trb.
        debug!("Set address control transfer is received with address: {}", address);
    }
}
