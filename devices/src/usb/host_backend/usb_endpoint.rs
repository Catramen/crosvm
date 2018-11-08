// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex};

use usb::xhci::xhci_transfer::{XhciTransfer, XhciTransferState, XhciTransferType, TransferDirection};
use usb::xhci::scatter_gather_buffer::ScatterGatherBuffer;
use usb_util::types::{EndpointType, EndpointDirection, ENDPOINT_DIRECTION_OFFSET};
use usb_util::device_handle::DeviceHandle;
use usb_util::usb_transfer::{UsbTransfer, BulkTransferBuffer, TransferStatus, bulk_transfer, interrupt_transfer};
use super::utils::{submit_transfer, update_state};
use usb::async_job_queue::AsyncJobQueue;

/// Isochronous, Bulk or Interrupt endpoint.
pub struct UsbEndpoint {
    job_queue: Arc<AsyncJobQueue>,
    device_handle: Arc<Mutex<DeviceHandle>>,
    endpoint_number: u8,
    direction: EndpointDirection,
    ty: EndpointType,
}

impl UsbEndpoint {
    pub fn new(job_queue: Arc<AsyncJobQueue>,
               device_handle: Arc<Mutex<DeviceHandle>>,
               endpoint_number: u8,
               direction: EndpointDirection,
               ty: EndpointType
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

    pub fn match_ep(&self, endpoint_number: u8, dir: &TransferDirection) -> bool {
        if self.endpoint_number != endpoint_number {
            return false;
        }
        match self.direction {
            EndpointDirection::HostToDevice => {
                if *dir == TransferDirection::Out {
                    true
                } else {
                    false
                }
            }
            EndpointDirection::DeviceToHost => {
                if *dir == TransferDirection::In {
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn handle_transfer(&self, transfer: XhciTransfer) {
        let buffer = match transfer.get_transfer_type() {
            XhciTransferType::Normal(buffer) => buffer,
            _ => {
                error!("Wrong transfer type, not handled.");
                transfer.on_transfer_complete(&TransferStatus::Error, 0);
                return;
            },
        };

        match self.ty {
            EndpointType::Bulk => {
                self.handle_bulk_transfer(transfer, buffer);
            },
            EndpointType::Interrupt => {
                self.handle_interrupt_transfer(transfer, buffer);
            },
            _ => {
                transfer.on_transfer_complete(&TransferStatus::Error, 0);
            }
        }
    }

    fn handle_bulk_transfer(&self, xhci_transfer: XhciTransfer, buffer: ScatterGatherBuffer) {
        let usb_transfer = bulk_transfer(self.ep_addr(), 0, buffer.len());
        self.do_handle_transfer(xhci_transfer, usb_transfer, buffer);
    }

    fn handle_interrupt_transfer(&self, xhci_transfer: XhciTransfer, buffer: ScatterGatherBuffer) {
        let usb_transfer = interrupt_transfer(self.ep_addr(), 0, buffer.len());
        self.do_handle_transfer(xhci_transfer, usb_transfer, buffer);
    }

    fn do_handle_transfer(&self, xhci_transfer: XhciTransfer,
                       mut usb_transfer: UsbTransfer<BulkTransferBuffer>, buffer: ScatterGatherBuffer) {
        let xhci_transfer = Arc::new(xhci_transfer);
        let tmp_transfer = xhci_transfer.clone();
        match self.direction {
            EndpointDirection::HostToDevice => {
                // Read data from ScatterGatherBuffer to a continuous memory.
                buffer.read(usb_transfer.mut_buffer().mut_slice());
                debug!("out transfer ep_addr {:#x}, buffer len {}, data {:#x?}", self.ep_addr(), buffer.len(), usb_transfer.mut_buffer().mut_slice());
                xhci_transfer.print();
                usb_transfer.set_callback(move |t: UsbTransfer<BulkTransferBuffer>| {
                    debug!("out transfer calllback");
                    update_state(&xhci_transfer, &t);
                    let state = xhci_transfer.state().lock().unwrap();
                    match *state {
                        XhciTransferState::Cancelled => {
                            debug!("transfer has been cancelled");
                            drop(state);
                            xhci_transfer.on_transfer_complete(&TransferStatus::Cancelled, 0);
                        }
                        XhciTransferState::Completed => {
                            xhci_transfer.print();
                            let status = t.status();
                            let actual_length = t.actual_length();
                            drop(state);
                            xhci_transfer.on_transfer_complete(&status, actual_length as u32);
                        }
                        _ => {
                            panic!("should not take this branch");
                        }
                    }
                });
                submit_transfer(&self.job_queue ,tmp_transfer, &self.device_handle, usb_transfer);
            },
            EndpointDirection::DeviceToHost => {
                debug!("in transfer ep_addr {:#x}, buffer len {}", self.ep_addr(), buffer.len());
                xhci_transfer.print();
                let addr = self.ep_addr();
                usb_transfer.set_callback(move |t: UsbTransfer<BulkTransferBuffer>| {
                    xhci_transfer.print();
                    debug!("ep {:#x} in transfer data {:?}", addr,  t.buffer().slice());
                    update_state(&xhci_transfer, &t);
                    let state = xhci_transfer.state().lock().unwrap();
                    match *state {
                        XhciTransferState::Cancelled => {
                            debug!("transfer has been cancelled");
                            drop(state);
                            xhci_transfer.on_transfer_complete(&TransferStatus::Cancelled, 0);
                        }
                        XhciTransferState::Completed => {
                            let status = t.status();
                            let actual_length = t.actual_length() as usize;
                            let copied_length = buffer.write(t.buffer().slice());
                            let actual_length = {
                                if actual_length > copied_length {
                                    copied_length
                                } else {
                                    actual_length
                                }
                            };
                            drop(state);
                            xhci_transfer.on_transfer_complete(&status, actual_length as u32);
                        }
                        _ => {
                            panic!("should not take this branch");
                        }
                    }

                });

                submit_transfer(&self.job_queue, tmp_transfer, &self.device_handle, usb_transfer);
            },
        }
    }
}
