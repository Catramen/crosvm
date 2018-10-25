// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::mem;
use std::sync::{Arc, Mutex};

use usb::async_job_queue::AsyncJobQueue;
use usb::xhci::xhci_transfer::{XhciTransfer, XhciTransferState};
use usb_util::device_handle::DeviceHandle;
use usb_util::usb_transfer::{TransferStatus, UsbTransfer, UsbTransferBuffer};

pub fn update_state<T: UsbTransferBuffer>(
    xhci_transfer: &Arc<XhciTransfer>,
    usb_transfer: &UsbTransfer<T>,
) {
    let status = usb_transfer.status();
    let mut state = xhci_transfer.state().lock().unwrap();

    if status == TransferStatus::Cancelled {
        *state = XhciTransferState::Cancelled;
        return;
    }

    match *state {
        XhciTransferState::Cancelling => {
            *state = XhciTransferState::Cancelled;
        }
        XhciTransferState::Submitted { cancel_callback: _ } => {
            *state = XhciTransferState::Completed;
        }
        _ => {
            error!("tansfer is in an wrong state");
            // We consider this completed to avoid guest kernel panic.
            *state = XhciTransferState::Completed;
        }
    }
}
/// Helper function to submit usb_transfer to device handle.
pub fn submit_transfer<T: UsbTransferBuffer>(
    job_queue: &Arc<AsyncJobQueue>,
    xhci_transfer: Arc<XhciTransfer>,
    device_handle: &Arc<Mutex<DeviceHandle>>,
    usb_transfer: UsbTransfer<T>,
) {
    let transfer_status = {
        // We need to hold the lock to avoid race condition.
        // While we are trying to submit the transfer, another thread might want to cancel the same
        // transfer. Holding the lock here makes sure one of them is cancelled.
        let mut state = xhci_transfer.state().lock().unwrap();
        match mem::replace(&mut *state, XhciTransferState::Cancelled) {
            XhciTransferState::Created => {
                let canceller = usb_transfer.get_canceller();
                // TODO(jkwang) refactor canceller to return Cancel::Ok or Cancel::Err.
                let cancel_callback = Box::new(move || match canceller.try_cancel() {
                    true => debug!("cancel issued to libusb backend"),
                    false => debug!("fail to cancel"),
                });
                *state = XhciTransferState::Submitted { cancel_callback };
                match device_handle
                    .lock()
                    .unwrap()
                    .submit_async_transfer(usb_transfer)
                {
                    Err(e) => {
                        error!("fail to submit transfer {:?}", e);
                        *state = XhciTransferState::Completed;
                        TransferStatus::NoDevice
                    }
                    // If it's submitted, we don't need to send on_transfer_complete now.
                    Ok(_) => return,
                }
            }
            XhciTransferState::Cancelled => {
                warn!("Transfer is already cancelled");
                TransferStatus::Cancelled
            }
            _ => {
                // The transfer could not be in the following states:
                // Submitted: A transfer should only be submitted once.
                // Cancelling: Transfer is cancelling only when it's submitted and someone is
                // trying to cancel it.
                // Completed: A completed transfer should not be submitted again.
                panic!("there is a bug");
            }
        }
    };
    // We are holding locks to of backends, we want to call on_transfer_complete
    // without any lock.
    job_queue.queue_job(move || {
        xhci_transfer.on_transfer_complete(&transfer_status, 0);
    });
}
