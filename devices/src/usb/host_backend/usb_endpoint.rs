// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cmp;
use std::sync::Arc;
use sync::Mutex;

use super::utils::{submit_transfer, update_state};
use usb::async_job_queue::AsyncJobQueue;
use usb::error::Result;
use usb::xhci::scatter_gather_buffer::ScatterGatherBuffer;
use usb::xhci::xhci_transfer::{
    TransferDirection, XhciTransfer, XhciTransferState, XhciTransferType,
};
use usb_util::device_handle::DeviceHandle;
use usb_util::types::{EndpointDirection, EndpointType, ENDPOINT_DIRECTION_OFFSET};
use usb_util::usb_transfer::{
    bulk_transfer, interrupt_transfer, BulkTransferBuffer, TransferStatus, UsbTransfer,
};

/// Isochronous, Bulk or Interrupt endpoint.
pub struct UsbEndpoint {
    job_queue: Arc<AsyncJobQueue>,
    device_handle: Arc<Mutex<DeviceHandle>>,
    endpoint_number: u8,
    direction: EndpointDirection,
    ty: EndpointType,
}

impl UsbEndpoint {
    /// Create new endpoing. This function will panic if endpoint type is control.
    pub fn new(
        job_queue: Arc<AsyncJobQueue>,
        device_handle: Arc<Mutex<DeviceHandle>>,
        endpoint_number: u8,
        direction: EndpointDirection,
        ty: EndpointType,
    ) -> UsbEndpoint {
        assert!(ty != EndpointType::Control);
        UsbEndpoint {
            job_queue,
            device_handle,
            endpoint_number,
            direction,
            ty,
        }
    }

    fn ep_addr(&self) -> u8 {
        self.endpoint_number | ((self.direction as u8) << ENDPOINT_DIRECTION_OFFSET)
    }

    /// Returns true is this endpoint matches number and direction.
    pub fn match_ep(&self, endpoint_number: u8, dir: TransferDirection) -> bool {
        let self_dir = match self.direction {
            EndpointDirection::HostToDevice => TransferDirection::Out,
            EndpointDirection::DeviceToHost => TransferDirection::In,
        };
        self.endpoint_number == endpoint_number && self_dir == dir
    }

    /// Handle a xhci transfer.
    pub fn handle_transfer(&self, transfer: XhciTransfer) -> Result<()> {
        let buffer = match transfer.get_transfer_type()? {
            XhciTransferType::Normal(buffer) => buffer,
            _ => {
                error!("Wrong transfer type, not handled.");
                return transfer.on_transfer_complete(&TransferStatus::Error, 0);
            }
        };

        match self.ty {
            EndpointType::Bulk => {
                self.handle_bulk_transfer(transfer, buffer)?;
            }
            EndpointType::Interrupt => {
                self.handle_interrupt_transfer(transfer, buffer)?;
            }
            _ => {
                return transfer.on_transfer_complete(&TransferStatus::Error, 0);
            }
        }
        Ok(())
    }

    fn handle_bulk_transfer(
        &self,
        xhci_transfer: XhciTransfer,
        buffer: ScatterGatherBuffer,
    ) -> Result<()> {
        let usb_transfer = bulk_transfer(self.ep_addr(), 0, buffer.len()?);
        self.do_handle_transfer(xhci_transfer, usb_transfer, buffer)
    }

    fn handle_interrupt_transfer(
        &self,
        xhci_transfer: XhciTransfer,
        buffer: ScatterGatherBuffer,
    ) -> Result<()> {
        let usb_transfer = interrupt_transfer(self.ep_addr(), 0, buffer.len()?);
        self.do_handle_transfer(xhci_transfer, usb_transfer, buffer)
    }

    fn do_handle_transfer(
        &self,
        xhci_transfer: XhciTransfer,
        mut usb_transfer: UsbTransfer<BulkTransferBuffer>,
        buffer: ScatterGatherBuffer,
    ) -> Result<()> {
        let xhci_transfer = Arc::new(xhci_transfer);
        let tmp_transfer = xhci_transfer.clone();
        match self.direction {
            EndpointDirection::HostToDevice => {
                // Read data from ScatterGatherBuffer to a continuous memory.
                buffer.read(usb_transfer.buffer_mut().as_mut_slice())?;
                debug!(
                    "out transfer ep_addr {:#x}, buffer len {}, data {:#x?}",
                    self.ep_addr(),
                    buffer.len()?,
                    usb_transfer.buffer_mut().as_mut_slice()
                );
                usb_transfer.set_callback(move |t: UsbTransfer<BulkTransferBuffer>| {
                    debug!("out transfer callback");
                    update_state(&xhci_transfer, &t).unwrap();
                    let state = xhci_transfer.state().lock();
                    match *state {
                        XhciTransferState::Cancelled => {
                            debug!("transfer has been cancelled");
                            drop(state);
                            xhci_transfer
                                .on_transfer_complete(&TransferStatus::Cancelled, 0)
                                .unwrap();
                        }
                        XhciTransferState::Completed => {
                            let status = t.status();
                            let actual_length = t.actual_length();
                            drop(state);
                            xhci_transfer
                                .on_transfer_complete(&status, actual_length as u32)
                                .unwrap();
                        }
                        _ => {
                            panic!("should not take this branch");
                        }
                    }
                });
                submit_transfer(
                    &self.job_queue,
                    tmp_transfer,
                    &self.device_handle,
                    usb_transfer,
                )?;
            }
            EndpointDirection::DeviceToHost => {
                debug!(
                    "in transfer ep_addr {:#x}, buffer len {}",
                    self.ep_addr(),
                    buffer.len()?
                );
                let addr = self.ep_addr();
                usb_transfer.set_callback(move |t: UsbTransfer<BulkTransferBuffer>| {
                    debug!(
                        "ep {:#x} in transfer data {:?}",
                        addr,
                        t.buffer().as_slice()
                    );
                    update_state(&xhci_transfer, &t).unwrap();
                    let state = xhci_transfer.state().lock();
                    match *state {
                        XhciTransferState::Cancelled => {
                            debug!("transfer has been cancelled");
                            drop(state);
                            xhci_transfer
                                .on_transfer_complete(&TransferStatus::Cancelled, 0)
                                .unwrap();
                        }
                        XhciTransferState::Completed => {
                            let status = t.status();
                            let actual_length = t.actual_length() as usize;
                            let copied_length = buffer.write(t.buffer().as_slice()).unwrap();
                            let actual_length = cmp::min(actual_length, copied_length);
                            drop(state);
                            xhci_transfer
                                .on_transfer_complete(&status, actual_length as u32)
                                .unwrap();
                        }
                        _ => {
                            // update state is already invoked. This match should not be in any
                            // other state.
                            panic!("should not take this branch");
                        }
                    }
                });

                submit_transfer(
                    &self.job_queue,
                    tmp_transfer,
                    &self.device_handle,
                    usb_transfer,
                )?;
            }
        }
        Ok(())
    }
}
