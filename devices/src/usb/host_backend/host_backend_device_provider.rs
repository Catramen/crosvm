// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::Arc;

use super::context::Context;
use super::host_device::HostDevice;
use msg_socket::{MsgReceiver, MsgSender, MsgSocket};
use std::mem;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::UnixDatagram;
use std::time::Duration;
use sys_util::WatchingEvents;
use usb::async_job_queue::AsyncJobQueue;
use usb::event_loop::EventHandler;
use usb::event_loop::EventLoop;
use usb::xhci::usb_hub::UsbHub;
use usb::xhci::xhci_backend_device_provider::XhciBackendDeviceProvider;
use vm_control::{UsbControlCommand, UsbControlResult, UsbControlSocket};

const SOCKET_TIMEOUT_MS: u64 = 2000;

/// Host backend device provider is a xhci backend device provider that would provide pass through
/// devices.
pub enum HostBackendDeviceProvider {
    // The provider is created but not yet started.
    Created {
        sock: MsgSocket<UsbControlResult, UsbControlCommand>,
    },
    // The provider is started on an event loop.
    Started {
        inner: Arc<ProviderInner>,
    },
    // The provider has failed.
    Failed,
}

impl HostBackendDeviceProvider {
    pub fn new() -> (UsbControlSocket, HostBackendDeviceProvider) {
        let (child_sock, control_sock) = UnixDatagram::pair().unwrap();
        control_sock
            .set_write_timeout(Some(Duration::from_millis(SOCKET_TIMEOUT_MS)))
            .unwrap();
        control_sock
            .set_read_timeout(Some(Duration::from_millis(SOCKET_TIMEOUT_MS)))
            .unwrap();

        let provider = HostBackendDeviceProvider::Created {
            sock: MsgSocket::new(child_sock),
        };
        (MsgSocket::new(control_sock), provider)
    }
}

impl XhciBackendDeviceProvider for HostBackendDeviceProvider {
    fn start(&mut self, event_loop: Arc<EventLoop>, hub: Arc<UsbHub>) {
        match mem::replace(self, HostBackendDeviceProvider::Failed) {
            HostBackendDeviceProvider::Created { sock } => {
                let ctx = match Context::new(event_loop.clone()) {
                    Some(ctx) => ctx,
                    None => {
                        error!("could not create libusb context");
                        return;
                    }
                };
                let job_queue = AsyncJobQueue::init(&event_loop);

                let inner = Arc::new(ProviderInner::new(job_queue, ctx, sock, hub));
                let handler: Arc<EventHandler> = inner.clone();
                event_loop.add_event(
                    &inner.sock,
                    WatchingEvents::empty().set_read(),
                    Arc::downgrade(&handler),
                );
                *self = HostBackendDeviceProvider::Started { inner };
            }
            HostBackendDeviceProvider::Started { inner: _ } => {
                panic!("Host backend provider is already started");
            }
            HostBackendDeviceProvider::Failed => {
                panic!("Host backend provider is already failed");
            }
        }
    }

    fn keep_fds(&self) -> Vec<RawFd> {
        match self {
            HostBackendDeviceProvider::Created { sock } => vec![sock.as_raw_fd()],
            _ => {
                panic!(
                    "Trying to get keepfds when HostBackendDeviceProvider is not in created state"
                );
            }
        }
    }
}

/// ProviderInner listens to control socket.
pub struct ProviderInner {
    job_queue: Arc<AsyncJobQueue>,
    ctx: Context,
    sock: MsgSocket<UsbControlResult, UsbControlCommand>,
    usb_hub: Arc<UsbHub>,
}

impl ProviderInner {
    fn new(
        job_queue: Arc<AsyncJobQueue>,
        ctx: Context,
        sock: MsgSocket<UsbControlResult, UsbControlCommand>,
        usb_hub: Arc<UsbHub>,
    ) -> ProviderInner {
        ProviderInner {
            job_queue,
            ctx,
            sock,
            usb_hub,
        }
    }
}

impl EventHandler for ProviderInner {
    fn on_event(&self, _fd: RawFd) {
        let cmd = self.sock.recv().unwrap();
        match cmd {
            UsbControlCommand::AttachDevice {
                bus,
                addr,
                vid,
                pid,
                #[cfg(feature = "sandboxed-libusb")]
                fd,
            } => {
                let device = match self.ctx.get_device(bus, addr, vid, pid) {
                    Some(d) => d,
                    None => {
                        self.sock.send(&UsbControlResult::NoSuchDevice).unwrap();
                        return;
                    }
                };
                let device_handle = match device.open_fd(fd) {
                    Ok(handle) => handle,
                    Err(e) => {
                        error!("fail to open device {:?}", e);
                        self.sock.send(&UsbControlResult::FailedToOpenDevice).unwrap();
                        return;
                    }
                };
                let device = Box::new(HostDevice::new(
                    self.job_queue.clone(),
                    device,
                    device_handle,
                ));
                let port = self.usb_hub.connect_backend(device);
                match port {
                    Some(port) => {
                        self.sock.send(&UsbControlResult::Ok { port }).unwrap();
                    }
                    None => {
                        self.sock.send(&UsbControlResult::NoAvailablePort).unwrap();
                    }
                }
            }
            UsbControlCommand::DetachDevice { port } => {
                if self.usb_hub.disconnect_port(port) {
                    self.sock.send(&UsbControlResult::Ok { port }).unwrap();
                } else {
                    self.sock.send(&UsbControlResult::NoSuchDevice).unwrap();
                }
            }
            UsbControlCommand::ListDevice { port } => {
                let port_number = port;
                let result = match self.usb_hub.get_port(port_number) {
                    Some(port) => match *port.get_backend_device() {
                        Some(ref device) => {
                            let vid = device.get_vid();
                            let pid = device.get_pid();
                            UsbControlResult::Device {
                                port: port_number,
                                vid,
                                pid,
                            }
                        }
                        None => UsbControlResult::NoSuchDevice,
                    },
                    None => UsbControlResult::NoSuchPort,
                };
                self.sock.send(&result).unwrap();
            }
        }
    }
}
