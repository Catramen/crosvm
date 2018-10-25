// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex};

use usb::xhci::xhci_transfer::{XhciTransfer, XhciTransferType, TransferDirection};
use usb::xhci::scatter_gather_buffer::ScatterGatherBuffer;
use usb_util::types::{EndpointType, EndpointDirection, ENDPOINT_DIRECTION_OFFSET};
use usb_util::device_handle::DeviceHandle;
use usb_util::usb_transfer::{UsbTransfer, BulkTransferBuffer, TransferStatus, bulk_transfer, interrupt_transfer};

/// Isochronous, Bulk or Interrupt endpoint.
pub struct UsbEndpoint {
    device_handle: Arc<Mutex<DeviceHandle>>,
    endpoint_number: u8,
    direction: EndpointDirection,
    ty: EndpointType,
}

impl UsbEndpoint {
    pub fn new(device_handle: Arc<Mutex<DeviceHandle>>,
               endpoint_number: u8,
               direction: EndpointDirection,
               ty: EndpointType
               ) -> UsbEndpoint {
        assert!(ty != EndpointType::Control);
        UsbEndpoint {
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
                transfer.on_transfer_complete(TransferStatus::Error, 0);
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
                transfer.on_transfer_complete(TransferStatus::Error, 0);
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
                    xhci_transfer.print();
                    let status = t.status();
                    let actual_length = t.actual_length();
                    xhci_transfer.on_transfer_complete(status, actual_length as u32);
                });
                match self.device_handle.lock().unwrap().submit_async_transfer(usb_transfer) {
                    Err((e, _t)) => {
                        error!("fail to submit bulk transfer {:?}", e);
                        tmp_transfer.on_transfer_complete(TransferStatus::Error, 0);
                    },
                    _ =>{},
                }
            },
            EndpointDirection::DeviceToHost => {
                debug!("in transfer ep_addr {:#x}, buffer len {}", self.ep_addr(), buffer.len());
                xhci_transfer.print();
                let addr = self.ep_addr();
                usb_transfer.set_callback(move |t: UsbTransfer<BulkTransferBuffer>| {
                    xhci_transfer.print();
                    debug!("ep {:#x} in transfer data {:?}", addr,  t.buffer().slice());
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
                    xhci_transfer.on_transfer_complete(status, actual_length as u32);
                });
                match self.device_handle.lock().unwrap().submit_async_transfer(usb_transfer) {
                    Err((e, _t)) => {
                        error!("fail to submit bulk transfer {:?}", e);
                        tmp_transfer.on_transfer_complete(TransferStatus::Error, 0);
                    },
                    _ =>{},
                }
            },
        }
    }
}
