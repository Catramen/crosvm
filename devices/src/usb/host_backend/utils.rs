// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex};
use std::mem::{swap, drop};

use usb::xhci::xhci_transfer::{XhciTransfer, XhciTransferState};
use usb_util::device_handle::DeviceHandle;
use usb_util::usb_transfer::{UsbTransfer, UsbTransferBuffer, BulkTransferBuffer, TransferStatus};
use usb::async_job_queue::AsyncJobQueue;

/// Update transfer state, return true if it's cancelled.
pub fn update_state<T: UsbTransferBuffer>(xhci_transfer: &Arc<XhciTransfer>,
                                          usb_transfer: &UsbTransfer<T>) {
    let status = usb_transfer.status();
    let mut state = xhci_transfer.state().lock().unwrap();

    if status == TransferStatus::Cancelled {
        *state = XhciTransferState::Cancelled;
        return;
    }

    match *state {
        XhciTransferState::Cancelling => {
            *state = XhciTransferState::Cancelled;
        },
        XhciTransferState::Submitted(_) => {
            *state = XhciTransferState::Completed;
        }
        _ => {
            error!("tansfer is in an wrong state");
            // We consider this completed to avoid guest kernel panic.
            XhciTransferState::Completed;
        }
    }
}
/// Helper function to submit usb_transfer to device handle.
pub fn submit_transfer<T: UsbTransferBuffer>(job_queue: &Arc<AsyncJobQueue>,
                                             xhci_transfer: Arc<XhciTransfer>,
                                             device_handle: &Arc<Mutex<DeviceHandle>>,
                                             usb_transfer: UsbTransfer<T>) {
    let transfer_status = {
        // We need to hold the lock to avoid race condition.
        let mut state = xhci_transfer.state().lock().unwrap();
        let mut tmp = XhciTransferState::Cancelled;
        swap(&mut *state, &mut tmp);
        match tmp {
            XhciTransferState::Created => {
                let canceller = usb_transfer.get_canceller();
                let cancel_cb = Box::new(move || {
                    match canceller.try_cancel() {
                        true => debug!("cancel issued to libusb backend"),
                        false => debug!("fail to cancel"),
                    }
                });
                *state = XhciTransferState::Submitted(cancel_cb);
                match device_handle.lock().unwrap().submit_async_transfer(usb_transfer) {
                    Err(e) => {
                        error!("fail to submit transfer {:?}", e);
                        *state = XhciTransferState::Completed;
                        TransferStatus::NoDevice
                    },
                    // If it's submitted, we don't need to send on_transfer_complete now.
                    _ => return,
                }
            },
            XhciTransferState::Cancelled => {
                warn!("Transfer is already cancelled");
                TransferStatus::Cancelled
            },
            _ => {
                panic!("there is a bug");
            }
        }
    };
    // We are holding locks to of backends, we want to call on_transfer_complete
    // without any lock.
    job_queue.queue_job(
        move || {
            xhci_transfer.on_transfer_complete(&transfer_status, 0);
        }
    );
}

