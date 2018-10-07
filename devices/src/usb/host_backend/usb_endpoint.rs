// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex};

use usb::xhci::xhci_transfer::{XhciTransfer, EndpointDirection, XhciTransferType};
use usb::xhci::scatter_gather_buffer::ScatterGatherBuffer;
use usb_util::types::{EndpointType};
use usb_util::device_handle::DeviceHandle;
use usb_util::usb_transfer::{UsbTransfer, BulkTransferBuffer, TransferStatus, bulk_transfer};

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
        UsbEndpoint {
            device_handle,
            endpoint_number,
            direction,
            ty,
        }
    }

    pub fn match_ep(&self, endpoint_number: u8, dir: &EndpointDirection) -> bool {
        (self.endpoint_number == endpoint_number) && (self.direction == *dir)
    }

    pub fn handle_transfer(&self, transfer: XhciTransfer) {
        if self.ty != EndpointType::Bulk {
            warn!("Endpoint type is not supporetd");
            transfer.on_transfer_complete(TransferStatus::Error, 0);
        }
        match transfer.get_transfer_type() {
            XhciTransferType::Normal(buffer) => {
                self.handle_bulk_transfer(transfer, buffer);
            },
            _ => {
                error!("Wrong transfer type routed to Bulk endpoint.");

            },
        }
    }

    fn handle_bulk_transfer(&self, transfer: XhciTransfer, buffer: ScatterGatherBuffer) {
        let mut bulk_transfer = bulk_transfer(self.endpoint_number, 0, buffer.len());
        let transfer = Arc::new(transfer);
        let tmp_transfer = transfer.clone();
        match self.direction {
            EndpointDirection::In => {
                // Read data from ScatterGatherBuffer to a continuous memory.
                buffer.read(bulk_transfer.mut_buffer().mut_slice());
                bulk_transfer.set_callback(move |t: UsbTransfer<BulkTransferBuffer>| {
                    let status = t.status();
                    let actual_length = t.actual_length();
                    transfer.on_transfer_complete(status, actual_length as u32);
                });
                match self.device_handle.lock().unwrap().submit_async_transfer(bulk_transfer) {
                    Err((e, _t)) => {
                        error!("fail to submit bulk transfer {:?}", e);
                        tmp_transfer.on_transfer_complete(TransferStatus::Error, 0);
                    },
                    _ =>{},
                }
            },
            EndpointDirection::Out => {
                bulk_transfer.set_callback(move |t: UsbTransfer<BulkTransferBuffer>| {
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
                    transfer.on_transfer_complete(status, actual_length as u32);
                });
                match self.device_handle.lock().unwrap().submit_async_transfer(bulk_transfer) {
                    Err((e, _t)) => {
                        error!("fail to submit bulk transfer {:?}", e);
                        tmp_transfer.on_transfer_complete(TransferStatus::Error, 0);
                    },
                    _ =>{},
                }
            },
            _ => {
                error!("Wrong direction for bulk endpoint");
                transfer.on_transfer_complete(TransferStatus::Error, 0);
            }
        }
    }
}
