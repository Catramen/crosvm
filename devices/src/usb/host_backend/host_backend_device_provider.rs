// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::Arc;

use super::context::Context;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::UnixDatagram;
use sys_util::WatchingEvents;
use usb::event_loop::EventHandler;
use usb::xhci::xhci_backend_device_provider::XhciBackendDeviceProvider;
use std::time::Duration;
use super::host_device::HostDevice;
use usb::event_loop::EventLoop;
use usb::xhci::usb_hub::UsbHub;
use msg_socket::{MsgSocket, MsgReceiver, MsgSender};
use vm_control::{UsbControlCommand, UsbControlResult, UsbControlSocket};
use usb::async_job_queue::AsyncJobQueue;

const SOCKET_TIMEOUT_MS: u64 = 2000;

pub struct HostBackendDeviceProvider {
    sock: Option<MsgSocket<UsbControlResult, UsbControlCommand>>,
    inner: Option<Arc<ProviderInner>>,
}

impl HostBackendDeviceProvider {
    pub fn create() -> (
        UsbControlSocket,
        HostBackendDeviceProvider,
    ) {
        let (child_sock, control_sock) = UnixDatagram::pair().unwrap();
        control_sock
            .set_write_timeout(Some(Duration::from_millis(SOCKET_TIMEOUT_MS))).unwrap();
        control_sock
            .set_read_timeout(Some(Duration::from_millis(SOCKET_TIMEOUT_MS))).unwrap();

        let provider = HostBackendDeviceProvider {
            sock: Some(
                      MsgSocket::<UsbControlResult, UsbControlCommand>::new(child_sock)
                      ),
            inner: None,
        };
        (MsgSocket::<UsbControlCommand, UsbControlResult>::new(control_sock), provider)
    }
}

impl XhciBackendDeviceProvider for HostBackendDeviceProvider {
    fn start(&mut self, event_loop: Arc<EventLoop>, hub: Arc<UsbHub>) {
        if self.inner.is_some() {
            panic!("Host backend provider event loop is already set");
        }
        let event_fd = self.sock.as_ref().unwrap().as_ref().as_raw_fd();
        let inner = Arc::new(ProviderInner::new(
            self.sock.take().unwrap(),
            event_loop.clone(),
            hub,
        ));
        let handler: Arc<EventHandler> = inner.clone();
        debug!("event loop add event {} device provider", event_fd);
        event_loop.add_event(
            event_fd,
            WatchingEvents::empty().set_read(),
            Arc::downgrade(&handler),
        );
        self.inner = Some(inner);
    }

    fn keep_fds(&self) -> RawFd {
        self.sock.as_ref().unwrap().as_ref().as_raw_fd()
    }
}

/// ProviderInner listens to control socket.
struct ProviderInner {
    job_queue: Arc<AsyncJobQueue>,
    ctx: Context,
    sock: MsgSocket<UsbControlResult, UsbControlCommand>,
    usb_hub: Arc<UsbHub>,
}

impl ProviderInner {
    fn new(
        sock: MsgSocket<UsbControlResult, UsbControlCommand>,
        event_loop: Arc<EventLoop>,
        usb_hub: Arc<UsbHub>,
    ) -> ProviderInner {
        ProviderInner {
            job_queue: AsyncJobQueue::init(&event_loop),
            ctx: Context::new(event_loop),
            sock,
            usb_hub,
        }
    }
}

impl EventHandler for ProviderInner {
    fn on_event(&self, _fd: RawFd) {
        let cmd = self.sock.recv().unwrap();
        match cmd {
            UsbControlCommand::AttachDevice{ bus, addr, vid, pid, fd } => {
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
                        self.sock.send(&UsbControlResult::FailToOpenDevice).unwrap();
                        return;
                    }
                };
                let device = Box::new(HostDevice::new(self.job_queue.clone(),
                        device, device_handle));
                debug!("new host device created");
                let port = self.usb_hub.connect_backend(device);
                debug!("connected");
                match port {
                    Some(port) => {
                        self.sock.send(&UsbControlResult::Ok{port}).unwrap();
                    },
                    None => {
                        self.sock.send(&UsbControlResult::NoAvailablePort).unwrap();
                    }
                }
            },
            UsbControlCommand::DetachDevice{ port } => {
                if self.usb_hub.disconnect_port(port) {
                    self.sock.send(&UsbControlResult::Ok{port}).unwrap();
                } else {
                    self.sock.send(&UsbControlResult::NoSuchDevice).unwrap();
                }
            },
            UsbControlCommand::ListDevice{ port } => {
                let port_number = port;
                let result = match self.usb_hub.get_port(port_number) {
                    Some(port) => {
                        match *port.get_backend_device() {
                            Some(ref device) => {
                                let vid = device.get_vid();
                                let pid = device.get_pid();
                                UsbControlResult::Device{port: port_number, vid, pid}
                            }
                            None => {
                                UsbControlResult::NoSuchDevice
                            }
                        }
                    },
                    _ => UsbControlResult::NoSuchPort,
                };
                self.sock.send(&result).unwrap();
            },
        };
    }
}
