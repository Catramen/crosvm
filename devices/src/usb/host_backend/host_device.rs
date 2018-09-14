// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex};

use usb::xhci::xhci_backend_device::{XhciBackendDevice, UsbDeviceAddress};
use usb::xhci::xhci_transfer::{XhciTransfer, XhciTransferType, TransferStatus};
use usb_util::device_handle::DeviceHandle;
use usb_util::usb_transfer::{UsbTransfer, ControlTransferBuffer, control_transfer};
use usb_util::types::ControlRequestDataPhaseTransferDirection;
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
    device_handle: Arc<Mutex<DeviceHandle>>,
    ctl_ep_state: Mutex<ControlEndpointState>,
    control_transfer: Arc<Mutex<Option<UsbTransfer<ControlTransferBuffer>>>>,
    claimed_interface: Vec<i32>,
    host_claimed_interface: Vec<i32>,
}

impl HostDevice {
    pub fn new(handle: Arc<Mutex<DeviceHandle>>) -> HostDevice {
        HostDevice {
            endpoints: vec![],
            device_handle: handle,
            ctl_ep_state: Mutex::new(ControlEndpointState::StatusStage),
            control_transfer: Arc::new(Mutex::new(Some(control_transfer(0)))),
            claimed_interface: vec![],
            host_claimed_interface: vec![],
        }
    }

    fn handle_control_transfer(&self, transfer: XhciTransfer) {
        if self.control_transfer.lock().unwrap().is_none() {
            // Yes, we can cache and handle transfers later. But it's easier to not submit transfer
            // before last transfer is handled.
            panic!("Last control transfer has not yet finished.");
        }
        match transfer.get_transfer_type() {
            XhciTransferType::SetupStage(ref setup) => {
                if (*self.ctl_ep_state.lock().unwrap()) != ControlEndpointState::StatusStage {
                    error!("Control endpoing is in an inconsistant state");
                    return;
                }
                let mut locked = self.control_transfer.lock().unwrap();
                {
                    let request_setup = &mut locked.as_mut().unwrap().buffer().setup_buffer;
                    *request_setup = setup.clone();
                }
                // If the control transfer is device to host, we can go ahead submit the
                // transfer right now.
                if setup.get_direction().unwrap() ==
                    ControlRequestDataPhaseTransferDirection::DeviceToHost {
                        let mut control_transfer = locked.take().unwrap();
                        let myct = self.control_transfer.clone();
                        control_transfer.set_callback(move |t: UsbTransfer<ControlTransferBuffer>| {
                            (*myct.lock().unwrap()) = Some(t);
                            // TODO(jkwang) transfer completed. How is that?
                            //transfer.on_transfer_complete();
                        });
                        self.device_handle.lock().unwrap().submit_async_transfer(control_transfer);
                } else {
                    // TODO(jkwang) really completed.
                    //transfer.on_transfer_complete();
                }
                *self.ctl_ep_state.lock().unwrap() = ControlEndpointState::SetupStage;
            },
            XhciTransferType::DataStage(ref buffer) => {
                 if (*self.ctl_ep_state.lock().unwrap()) != ControlEndpointState::SetupStage {
                    error!("Control endpoing is in an inconsistant state");
                    return;
                 }
                 let mut locked = self.control_transfer.lock().unwrap();
                 let control_transfer = locked.as_mut().unwrap();
                 let tbuffer = control_transfer.buffer();
                 let request_setup = &tbuffer.setup_buffer;
                 match request_setup.get_direction() {
                         Some(ControlRequestDataPhaseTransferDirection::HostToDevice) => {
                         },
                         Some(ControlRequestDataPhaseTransferDirection::DeviceToHost) => {
                             // For device to host transfer, it's already handled in setup stage.
                             // As ScatterGatherBuffer implementation handles buffer size, we can
                             // copy all buffer.
                             buffer.write(&tbuffer.data_buffer);
                         },
                         _ => error!("Unknown transfer direction!"),
                 }

                 *self.ctl_ep_state.lock().unwrap() = ControlEndpointState::DataStage;

            },
            XhciTransferType::StatusStage => {
                if (*self.ctl_ep_state.lock().unwrap()) == ControlEndpointState::StatusStage {
                    error!("Control endpoing is in an inconsistant state");
                    return;
                }
                *self.ctl_ep_state.lock().unwrap() = ControlEndpointState::StatusStage;
            },
            _ => {
                panic!("Non control transfer sent to control endpoint");
            }
        }
    }

    fn set_config(&self) {
    }

    fn release_interface(&self) {
    }

    fn set_interface(&self) {
    }

    fn delete_all_endpoints(&mut self) {
        self.endpoints = vec![];
    }
}

impl XhciBackendDevice for HostDevice {
    fn submit_transfer(&self, transfer: XhciTransfer) {
        if transfer.get_endpoint_number() == 0 {
            self.handle_control_transfer(transfer);
            return;
        }
        for ep in &self.endpoints {
            if ep.match_ep(transfer.get_endpoint_number(), transfer.get_endpoint_dir()) {
                ep.handle_transfer(transfer);
                return;
            }
        }
        warn!("Could not find endpoint for transfer");
        transfer.on_transfer_complete(TransferStatus::Error, 0);
    }

    fn set_address(&self, address: UsbDeviceAddress) {
    }
}
