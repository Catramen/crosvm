// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex};

use super::context::Context;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::UnixDatagram;
use std::{self, fmt, io};
use sys_util::WatchingEvents;
use usb::event_loop::EventHandler;
use usb::xhci::xhci_backend_device::{UsbDeviceAddress, XhciBackendDevice};
use usb::xhci::xhci_backend_device_provider::XhciBackendDeviceProvider;
use usb_util::libusb_context::LibUsbContext;
use usb_util::libusb_device::LibUsbDevice;
use std::time::Duration;
use byteorder::{LittleEndian, NativeEndian, ByteOrder};
use super::host_device::HostDevice;
use usb::event_loop::EventLoop;
use usb::xhci::usb_ports::UsbPorts;
use msg_socket::{MsgOnSocket, MsgError, MsgResult, MsgSocket, MsgReceiver, MsgSender};

const SOCKET_TIMEOUT_MS: u64 = 2000;

#[derive(MsgOnSocket)]
enum UsbControlCommand {
    AttachDevice{bus: u8, addr: u8},
    DetachDevice{port: u8},
    ListDevice{port: u8},
}

#[derive(MsgOnSocket)]
pub enum UsbControlResult {
    Ok{port: u8},
    NoAvailablePort,
    NoSuchDevice,
    Device{vid: u16, pid: u16},
}

pub struct HostBackendDeviceProvider {
    sock: Option<MsgSocket<UsbControlResult, UsbControlCommand>>,
    inner: Option<Arc<ProviderInner>>,
}

impl HostBackendDeviceProvider {
    pub fn create() -> (
        HostBackendDeviceProviderControlSocket,
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
    fn start(&mut self, event_loop: EventLoop, ports: Arc<Mutex<UsbPorts>>) {
        if self.inner.is_some() {
            panic!("Host backend provider event loop is already set");
        }
        let event_fd = self.sock.as_ref().unwrap().as_ref().as_raw_fd();
        let inner = Arc::new(ProviderInner::new(
            self.sock.take().unwrap(),
            event_loop.clone(),
            ports,
        ));
        let handler: Arc<EventHandler> = inner.clone();
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

pub type HostBackendDeviceProviderControlSocket = MsgSocket<UsbControlCommand, UsbControlResult>;

/*
pub struct HostBackendDeviceProviderController {
    sock: MsgSocket<Command, CommandResult>,
}

impl HostBackendDeviceProviderController {
    fn new(sock: UnixDatagram) -> Self {
        HostBackendDeviceProviderController {
            control_sock: MsgSocket::<Command, CommandResult>::new(sock)
        }
    }
    
    pub fn attach_device(&self, bus: u8, addr: u8) -> MsgResult<u8> {
        self.sock.send(&Command::AttachDevice{bus, addr})?;
        let result = self.sock.recv()?;
        Ok(result)
    }
    pub fn detach_device(&self, port: u8) -> MsgResult<u8> {
        self.sock.send(&Command::DetachDevice{port})?;
        let result = self.sock.recv()?;
        if let CommandResult::Ok(port) = result {
            Ok(port);
        } else {
            panic!("Wrong result from usb host backend provider");
        }
    }

    pub fn list_device(&self, port: u8) -> MsgResult<(u16, u16)> {
        self.sock.send(&Command::ListDevice{port})?;
        let result = self.sock.recv()?;
        if let CommandResult::
    }
}*/

/// ProviderInner listens to control socket.
struct ProviderInner {
    ctx: Context,
    sock: MsgSocket<UsbControlResult, UsbControlCommand>,
    usb_ports: Arc<Mutex<UsbPorts>>,
}

impl ProviderInner {
    fn new(
        sock: MsgSocket<UsbControlResult, UsbControlCommand>,
        event_loop: EventLoop,
        ports: Arc<Mutex<UsbPorts>>,
    ) -> ProviderInner {
        ProviderInner {
            ctx: Context::new(event_loop),
            sock,
            usb_ports: ports,
        }
    }
}

impl EventHandler for ProviderInner {
    fn on_event(&self, fd: RawFd) {
        let cmd = self.sock.recv().unwrap();
        match cmd {
            UsbControlCommand::AttachDevice{ bus, addr } => {
                let device = self.ctx.get_device(bus, addr).unwrap();
                let device = Arc::new(Mutex::new(HostDevice::new(device)));
                let port = self
                    .usb_ports
                    .lock()
                    .unwrap()
                    .connect_backend(device);
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
                self.usb_ports.lock().unwrap().disconnect_backend(port);
                self.sock.send(&UsbControlResult::Ok{port}).unwrap();
            },
            UsbControlCommand::ListDevice{ port } => {
                let result = match self.usb_ports.lock().unwrap().get_backend_for_port(port) {
                    Some(device) => {
                        let vid = device.lock().unwrap().get_vid();
                        let pid = device.lock().unwrap().get_pid();
                        UsbControlResult::Device{vid, pid}
                    },
                    _ => UsbControlResult::NoSuchDevice,
                };
                self.sock.send(&result).unwrap();
            },
        };
    }
}
