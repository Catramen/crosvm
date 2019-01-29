// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::mem::drop;
use std::sync::Arc;
use sync::Mutex;

use super::usb_endpoint::UsbEndpoint;
use super::utils::{submit_transfer, update_state};
use std::collections::HashMap;
use usb::async_job_queue::AsyncJobQueue;
use usb::error::{Error, Result};
use usb::event_loop::FailHandle;
use usb::xhci::scatter_gather_buffer::ScatterGatherBuffer;
use usb::xhci::xhci_backend_device::{UsbDeviceAddress, XhciBackendDevice};
use usb::xhci::xhci_controller::XhciFailHandle;
use usb::xhci::xhci_transfer::{XhciTransfer, XhciTransferState, XhciTransferType};
use usb_util::device_handle::DeviceHandle;
use usb_util::error::Error as LibUsbError;
use usb_util::libusb_device::LibUsbDevice;
use usb_util::types::{
    ControlRequestDataPhaseTransferDirection, ControlRequestRecipient, ControlRequestType,
    StandardControlRequest, UsbRequestSetup,
};
use usb_util::usb_transfer::{
    control_transfer, ControlTransferBuffer, TransferStatus, UsbTransfer,
};

#[derive(PartialEq)]
pub enum ControlEndpointState {
    /// Control endpoint should receive setup stage next.
    SetupStage,
    /// Control endpoint should receive data stage next.
    DataStage,
    /// Control endpoint should receive status stage next.
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
    /// Analyze request setup.
    pub fn analyze_request_setup(
        request_setup: &UsbRequestSetup,
    ) -> Result<HostToDeviceControlRequest> {
        if request_setup.get_type().ok_or(Error::BadState)? == ControlRequestType::Standard
            && request_setup.get_recipient() == ControlRequestRecipient::Device
            && request_setup.get_standard_request() == Some(StandardControlRequest::SetAddress)
        {
            return Ok(HostToDeviceControlRequest::SetAddress);
        }
        if request_setup.get_type().ok_or(Error::BadState)? == ControlRequestType::Standard
            && request_setup.get_recipient() == ControlRequestRecipient::Device
            && request_setup.get_standard_request()
                == Some(StandardControlRequest::SetConfiguration)
        {
            return Ok(HostToDeviceControlRequest::SetConfig);
        };
        if request_setup.get_type().ok_or(Error::BadState)? == ControlRequestType::Standard
            && request_setup.get_recipient() == ControlRequestRecipient::Interface
            && request_setup.get_standard_request() == Some(StandardControlRequest::SetInterface)
        {
            return Ok(HostToDeviceControlRequest::SetInterface);
        };
        if request_setup.get_type().ok_or(Error::BadState)? == ControlRequestType::Standard
            && request_setup.get_recipient() == ControlRequestRecipient::Endpoint
            && request_setup.get_standard_request() == Some(StandardControlRequest::ClearFeature)
        {
            return Ok(HostToDeviceControlRequest::ClearFeature);
        };
        Ok(HostToDeviceControlRequest::Other)
    }
}

/// Host device is a device connected to host.
pub struct HostDevice {
    fail_handle: Arc<XhciFailHandle>,
    // Endpoints only contains data endpoints (1 to 30). Control transfers are handled at device
    // level.
    endpoints: Vec<UsbEndpoint>,
    device: LibUsbDevice,
    device_handle: Arc<Mutex<DeviceHandle>>,
    ctl_ep_state: ControlEndpointState,
    alt_settings: HashMap<u16, u16>,
    claimed_interfaces: Vec<i32>,
    host_claimed_interfaces: Vec<i32>,
    control_request_setup: UsbRequestSetup,
    buffer: Option<ScatterGatherBuffer>,
    job_queue: Arc<AsyncJobQueue>,
}

impl Drop for HostDevice {
    fn drop(&mut self) {
        self.release_interfaces();
    }
}

impl HostDevice {
    /// Create a new host device.
    pub fn new(
        fail_handle: Arc<XhciFailHandle>,
        job_queue: Arc<AsyncJobQueue>,
        device: LibUsbDevice,
        device_handle: DeviceHandle,
    ) -> HostDevice {
        let mut device = HostDevice {
            fail_handle,
            endpoints: vec![],
            device,
            device_handle: Arc::new(Mutex::new(device_handle)),
            ctl_ep_state: ControlEndpointState::SetupStage,
            alt_settings: HashMap::new(),
            claimed_interfaces: vec![],
            host_claimed_interfaces: vec![],
            control_request_setup: UsbRequestSetup::new(0, 0, 0, 0, 0),
            buffer: None,
            job_queue,
        };
        device
    }

    fn get_interface_number_of_active_config(&self) -> i32 {
        match self.device.get_active_config_descriptor() {
            Err(LibUsbError::NotFound) => {
                debug!("device is in unconfigured state");
                0
            }
            Err(e) => {
                // device might be disconnected now.
                error!("unexpected error {:?}", e);
                0
            }
            Ok(descriptor) => descriptor.bNumInterfaces as i32,
        }
    }
    fn detach_host_drivers(&mut self) {
        for i in 0..self.get_interface_number_of_active_config() {
            match self.device_handle.lock().kernel_driver_active(i) {
                Ok(true) => {
                    if let Err(e) = self.device_handle.lock().detach_kernel_driver(i as i32) {
                        error!("unexpected error {:?}", e);
                    } else {
                        debug!("host driver detached for interface {}", i);
                        self.host_claimed_interfaces.push(i);
                    }
                }
                Ok(false) => {
                    debug!("no driver attached");
                }
                Err(e) => {
                    error!("unexpected error {:?}", e);
                }
            }
        }
    }

    fn release_interfaces(&mut self) {
        for i in &self.claimed_interfaces {
            if let Err(e) = self.device_handle.lock().release_interface(*i) {
                error!("could not release interface {:?}", e);
            }
        }
        self.claimed_interfaces = Vec::new();
    }

    fn attach_host_drivers(&mut self) {
        for i in &self.host_claimed_interfaces {
            if let Err(e) = self.device_handle.lock().attach_kernel_driver(*i) {
                error!("could not attach host kernel {:?}", e);
            }
        }
    }

    fn handle_control_transfer(&mut self, transfer: XhciTransfer) -> Result<()> {
        let xhci_transfer = Arc::new(transfer);
        match xhci_transfer.get_transfer_type()? {
            XhciTransferType::SetupStage(setup) => {
                if self.ctl_ep_state != ControlEndpointState::SetupStage {
                    error!("Control endpoing is in an inconsistant state");
                    return Ok(());
                }
                debug!("setup stage setup buffer {:?}", setup);
                self.control_request_setup = setup;
                xhci_transfer.on_transfer_complete(&TransferStatus::Completed, 0)?;
                self.ctl_ep_state = ControlEndpointState::DataStage;
            }
            XhciTransferType::DataStage(buffer) => {
                if self.ctl_ep_state != ControlEndpointState::DataStage {
                    error!("Control endpoing is in an inconsistant state");
                    return Ok(());
                }
                self.buffer = Some(buffer);
                xhci_transfer.on_transfer_complete(&TransferStatus::Completed, 0)?;
                self.ctl_ep_state = ControlEndpointState::StatusStage;
            }
            XhciTransferType::StatusStage => {
                if self.ctl_ep_state == ControlEndpointState::SetupStage {
                    error!("Control endpoing is in an inconsistant state");
                    return Ok(());
                }
                let buffer = self.buffer.take();
                match self.control_request_setup.get_direction() {
                    Some(ControlRequestDataPhaseTransferDirection::HostToDevice) => {
                        match HostToDeviceControlRequest::analyze_request_setup(
                            &self.control_request_setup,
                        )? {
                            HostToDeviceControlRequest::Other => {
                                let mut control_transfer = control_transfer(0);
                                control_transfer
                                    .buffer_mut()
                                    .set_request_setup(&self.control_request_setup);
                                if let Some(buffer) = buffer {
                                    buffer.read(&mut control_transfer.buffer_mut().data_buffer)?;
                                }
                                let tmp_transfer = xhci_transfer.clone();
                                let callback = move |t: UsbTransfer<ControlTransferBuffer>| {
                                    update_state(&xhci_transfer, &t)?;
                                    let state = xhci_transfer.state().lock();
                                    match *state {
                                        XhciTransferState::Cancelled => {
                                            drop(state);
                                            xhci_transfer.on_transfer_complete(
                                                &TransferStatus::Cancelled,
                                                0,
                                            )?;
                                        }
                                        XhciTransferState::Completed => {
                                            let status = t.status();
                                            let actual_length = t.actual_length();
                                            drop(state);
                                            xhci_transfer.on_transfer_complete(
                                                &status,
                                                actual_length as u32,
                                            )?;
                                        }
                                        _ => {
                                            // update_state is already invoked before match. This
                                            // transfer  could only be `cancelled` or `compeleted`. Any
                                            // other states means there is a bug in crosvm
                                            // implemetation.
                                            error!("should not take this branch");
                                            return Err(Error::BadState);
                                        }
                                    }
                                    Ok(())
                                };
                                let fail_handle = self.fail_handle.clone();
                                control_transfer.set_callback(
                                    move |t: UsbTransfer<ControlTransferBuffer>| match callback(t) {
                                        Ok(_) => {}
                                        Err(e) => {
                                            error!("control transfer callback failed {:?}", e);
                                            fail_handle.fail();
                                        }
                                    },
                                );
                                submit_transfer(
                                    self.fail_handle.clone(),
                                    &self.job_queue,
                                    tmp_transfer,
                                    &self.device_handle,
                                    control_transfer,
                                )?;
                            }
                            HostToDeviceControlRequest::SetAddress => {
                                debug!("host device handling set address");
                                let addr = self.control_request_setup.value as u32;
                                self.set_address(addr);
                                xhci_transfer
                                    .on_transfer_complete(&TransferStatus::Completed, 0)?;
                            }
                            HostToDeviceControlRequest::SetConfig => {
                                debug!("host device handling set config");
                                let status = self.set_config()?;
                                xhci_transfer.on_transfer_complete(&status, 0)?;
                            }
                            HostToDeviceControlRequest::SetInterface => {
                                debug!("host device handling set interface");
                                let status = self.set_interface()?;
                                xhci_transfer.on_transfer_complete(&status, 0)?;
                            }
                            HostToDeviceControlRequest::ClearFeature => {
                                debug!("host device handling clear feature");
                                let status = self.clear_feature()?;
                                xhci_transfer.on_transfer_complete(&status, 0)?;
                            }
                        };
                    }
                    Some(ControlRequestDataPhaseTransferDirection::DeviceToHost) => {
                        let mut control_transfer = control_transfer(0);
                        control_transfer
                            .buffer_mut()
                            .set_request_setup(&self.control_request_setup);
                        let tmp_transfer = xhci_transfer.clone();
                        let callback = move |t: UsbTransfer<ControlTransferBuffer>| {
                            debug!("setup token control transfer callback invoked");
                            update_state(&xhci_transfer, &t)?;
                            let state = xhci_transfer.state().lock();
                            match *state {
                                XhciTransferState::Cancelled => {
                                    debug!("transfer cancelled");
                                    drop(state);
                                    xhci_transfer
                                        .on_transfer_complete(&TransferStatus::Cancelled, 0)?;
                                }
                                XhciTransferState::Completed => {
                                    let status = t.status();
                                    let actual_length = t.actual_length();
                                    if let Some(ref buffer) = buffer {
                                        let bytes = buffer.write(&t.buffer().data_buffer)? as u32;
                                        debug!(
                                            "transfer completed bytes: {} actual length {}",
                                            bytes, actual_length
                                        );
                                    }
                                    drop(state);
                                    xhci_transfer.on_transfer_complete(&status, 0)?;
                                }
                                _ => {
                                    // update_state is already invoked before this match.
                                    // Any other states indicates a bug in crosvm.
                                    error!("should not take this branch");
                                    return Err(Error::BadState);
                                }
                            }
                            Ok(())
                        };
                        let fail_handle = self.fail_handle.clone();
                        control_transfer.set_callback(
                            move |t: UsbTransfer<ControlTransferBuffer>| match callback(t) {
                                Ok(_) => {}
                                Err(e) => {
                                    error!("control transfer callback failed {:?}", e);
                                    fail_handle.fail();
                                }
                            },
                        );
                        submit_transfer(
                            self.fail_handle.clone(),
                            &self.job_queue,
                            tmp_transfer,
                            &self.device_handle,
                            control_transfer,
                        )?;
                    }
                    None => error!("Unknown transfer direction!"),
                }

                self.ctl_ep_state = ControlEndpointState::SetupStage;
            }
            _ => {
                // Non control transfer should not be handled in this function.
                panic!("Non control transfer sent to control endpoint. There is a crosvm bug.");
            }
        }
        Ok(())
    }

    fn set_config(&mut self) -> Result<TransferStatus> {
        // It's a standard, set_config, device request.
        let config = (self.control_request_setup.value & 0xff) as i32;
        debug!(
            "Set config control transfer is received with config: {}",
            config
        );
        self.release_interfaces();
        let cur_config = self
            .device_handle
            .lock()
            .get_active_configuration()
            .map_err(err_msg!(Error::Unknown))?;
        debug!("current config is: {}", cur_config);
        if config != cur_config {
            self.device_handle
                .lock()
                .set_active_configuration(config)
                .map_err(err_msg!(Error::Unknown))?;
        }
        self.claim_interfaces();
        self.create_endpoints()?;
        Ok(TransferStatus::Completed)
    }

    fn set_interface(&mut self) -> Result<TransferStatus> {
        debug!("set interface");
        // It's a standard, set_interface, interface request.
        let interface = self.control_request_setup.index;
        let alt_setting = self.control_request_setup.value;
        self.device_handle
            .lock()
            .set_interface_alt_setting(interface as i32, alt_setting as i32)
            .map_err(err_msg!(Error::Unknown))?;
        self.alt_settings.insert(interface, alt_setting);
        self.create_endpoints()?;
        Ok(TransferStatus::Completed)
    }

    fn clear_feature(&mut self) -> Result<TransferStatus> {
        debug!("clear feature");
        let request_setup = &self.control_request_setup;
        // It's a standard, clear_feature, endpoint request.
        const STD_FEATURE_ENDPOINT_HALT: u16 = 0;
        if request_setup.value == STD_FEATURE_ENDPOINT_HALT {
            self.device_handle
                .lock()
                .clear_halt(request_setup.index as u8)
                .map_err(err_msg!(Error::Unknown))?;
        }
        Ok(TransferStatus::Completed)
    }

    fn claim_interfaces(&mut self) {
        for i in 0..self.get_interface_number_of_active_config() {
            match self.device_handle.lock().claim_interface(i) {
                Ok(()) => {
                    debug!("claimed interface {}", i);
                    self.claimed_interfaces.push(i);
                }
                Err(e) => {
                    error!("unable to claim interface {}, error {:?}", i, e);
                }
            }
        }
    }

    fn create_endpoints(&mut self) -> Result<()> {
        self.endpoints = Vec::new();
        let config_descriptor = match self.device.get_active_config_descriptor() {
            Err(e) => {
                error!("device might be disconnected {:?}", e);
                return Ok(());
            }
            Ok(descriptor) => descriptor,
        };
        for i in &self.claimed_interfaces {
            let alt_setting = self.alt_settings.get(&(*i as u16)).unwrap_or(&0);
            let interface = config_descriptor
                .get_interface_descriptor(*i as u8, *alt_setting as i32)
                .ok_or(Error::Unknown)?;
            for ep_idx in 0..interface.bNumEndpoints {
                let ep_dp = interface
                    .endpoint_descriptor(ep_idx)
                    .ok_or(Error::Unknown)?;
                let ep_num = ep_dp.get_endpoint_number();
                if ep_num == 0 {
                    debug!("endpoint 0 in endpoint descriptors");
                    continue;
                }
                let direction = ep_dp.get_direction();
                let ty = ep_dp.get_endpoint_type().ok_or(Error::Unknown)?;
                self.endpoints.push(UsbEndpoint::new(
                    self.fail_handle.clone(),
                    self.job_queue.clone(),
                    self.device_handle.clone(),
                    ep_num,
                    direction,
                    ty,
                ));
            }
        }
        Ok(())
    }
}

impl XhciBackendDevice for HostDevice {
    fn get_vid(&self) -> u16 {
        match self.device.get_device_descriptor() {
            Ok(d) => d.idVendor,
            Err(e) => {
                error!("cannot get device descriptor {:?}", e);
                0
            }
        }
    }

    fn get_pid(&self) -> u16 {
        match self.device.get_device_descriptor() {
            Ok(d) => d.idProduct,
            Err(e) => {
                error!("cannot get device descriptor {:?}", e);
                0
            }
        }
    }

    fn submit_transfer(&mut self, transfer: XhciTransfer) -> Result<()> {
        if transfer.get_endpoint_number() == 0 {
            return self.handle_control_transfer(transfer);
        }
        for ep in &self.endpoints {
            if ep.match_ep(transfer.get_endpoint_number(), transfer.get_transfer_dir()) {
                return ep.handle_transfer(transfer);
            }
        }
        warn!("Could not find endpoint for transfer");
        transfer.on_transfer_complete(&TransferStatus::Error, 0)
    }

    fn set_address(&mut self, address: UsbDeviceAddress) {
        // It's a standard, set_address, device request. We do nothing here. As described in XHCI
        // spec. See set address command ring trb.
        debug!(
            "Set address control transfer is received with address: {}",
            address
        );
    }
}
