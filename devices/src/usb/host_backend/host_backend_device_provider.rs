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

const SOCKET_TIMEOUT_MS: u64 = 2000;
// size_of(command::attach: u8, bus: u8, addr: u8, padding: u8)
// size_of(command::detach/list: u8, port: u8)
// size_of(vid: u16, pid: u16)
const MSG_SIZE: usize = 3;

/// Errors for backend devices provider.
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
}
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::Io(ref e) => write!(f, "IO error {}.", e),
        }
    }
}

enum Command {
    AttachDevice = 0,
    DetachDevice = 1,
    ListDevice = 2,
}

pub struct HostBackendDeviceProvider {
    sock: Option<UnixDatagram>,
    inner: Option<Arc<ProviderInner>>,
}

impl HostBackendDeviceProvider {
    pub fn create() -> (
        HostBackendDeviceProviderController,
        HostBackendDeviceProvider,
    ) {
        let (child_sock, control_sock) = UnixDatagram::pair().map_err(Error::Io)?;
        control_sock
            .set_write_timeout(Some(Duration::from_millis(SOCKET_TIMEOUT_MS)))
            .map_err(Error::Io)?;
        control_sock
            .set_read_timeout(Some(Duration::from_millis(SOCKET_TIMEOUT_MS)))
            .map_err(Error::Io)?;
        let controller = HostBackendDeviceProviderController { control_sock };

        let provider = HostBackendDeviceProvider {
            event_loop: None,
            sock: Some(child_sock),
            inner: None,
        };
        (controller, provider)
    }
}

impl XhciBackendDeviceProvider for HostBackendDeviceProvider {
    fn start(&mut self, event_loop: EventLoop, ports: Arc<Mutex<UsbPorts>>) {
        if self.event_loop.is_some() {
            panic!("Event loop is already set.");
        }
        if self.inner.is_some() {
            panic!("Host backend provider event loop is already set");
        }
        let event_fd = self.sock.as_ref().unwrap().as_raw_fd();
        let inner = Arc::new(ProviderInner::new(
            self.sock.take(),
            event_loop.clone(),
            ports,
        ));
        event_loop.add_event(
            event_fd,
            WatchingEvents::new().set_read(),
            Arc::downgrade(&inner),
        );
        self.inner = Some(inner);
    }

    fn keep_fds(&self) -> RawFd {
        self.sock.as_raw_fd()
    }
}

pub struct HostBackendDeviceProviderController {
    control_sock: UnixDatagram,
}

impl HostBackendDeviceProviderController {
    fn new(sock: UnixDatagram) -> Self {
        HostBackendDeviceProviderController { control_sock: sock }
    }

    pub fn attach_device(&self, bus: u8, addr: u8) -> Result<(u8)> {
        let mut buf = [0; MSG_SIZE];
        NativeEndian::write_u8(&mut buf[0..], Command::AttachDevice as u8);
        NativeEndian::write_u8(&mut buf[1..], bus);
        NativeEndian::write_u8(&mut buf[2..], addr);
        handle_eintr!(self.sock.send(&buf))
            .map(|_| ())
            .map_err(Error::Io)?;

        handle_eintr!(self.sock.recv(&mut buf))
            .map(|_| ())
            .map_err(Error::Io)?;
        let port = NativeEndian::read_u8(&buf[0..]);
        Ok(port)
    }
    pub fn detach_device(&self, port: u8) -> Result<()> {
        let mut buf = [0; MSG_SIZE];
        NativeEndian::write_u8(&mut buf[0..], Command::DetachDevice as u8);
        NativeEndian::write_u8(&mut buf[1..], port);
        handle_eintr!(self.sock.send(&buf))
            .map(|_| ())
            .map_err(Error::Io)
    }

    pub fn list_device(&self, port: u8) -> Result<(u16, u16)> {
        let mut buf = [0; MSG_SIZE];
        NativeEndian::write_u8(&mut buf[0..], Command::ListDevice as u8);
        NativeEndian::write_u8(&mut buf[1..], port);
        handle_eintr!(self.sock.send(&buf))
            .map(|_| ())
            .map_err(Error::Io)?;

        handle_eintr!(self.sock.recv(&mut buf))
            .map(|_| ())
            .map_err(Error::Io)?;
        let vid = NativeEndian::read_u16(&buf[0..]);
        let pid = NativeEndian::read_u16(&buf[2..]);
        Ok((vid, pid))
    }
}

/// ProviderInner listens to control socket.
struct ProviderInner {
    ctx: Context,
    sock: UnixDatagram,
    usb_ports: Arc<Mutex<UsbPorts>>,
}

impl ProviderInner {
    fn new(
        sock: UnixDatagram,
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
        let mut buf = [0; MSG_SIZE];
        handle_eintr!(self.sock.recv(&mut buf))
            .map(|_| ())
            .map_err(Error::Io)?;
        let cmd = NativeEndian::read_u8(&buf[0..]);
        if cmd == Command::AttachDevice as u8 {
            let bus = NativeEndian::read_u8(&buf[1..]);
            let addr = NativeEndian::read_u8(&buf[2..]);

            let device = self.ctx.get_device(bus, addr).unwrap();
            let device = Arc::new(Mutex::new(HostDevice::new(device)));
            let port = self
                .usb_ports
                .lock()
                .unwrap()
                .connect_backend(device)
                .unwrap();
            NativeEndian::write_u8(&mut buf[0..], port);
            handle_eintr!(self.sock.send(&buf))
                .map(|_| ())
                .map_err(Error::Io)?;
        } else if cmd == Command::DetachDevice as u8 {
            let port = NativeEndian::read_u8(&buf[1..]);
            self.usb_ports.lock().unwrap().disconnect_backend(port)
        } else if cmd == Command::ListDevice as u8 {
            let port = NativeEndian::read_u8(&buf[1..]);
            let (vid, pid) = match self.get_backend_for_port(port) {
                Some(device) => {
                    let vid = device.lock().unwrap().get_vid();
                    let pid = device.lock().unwrap().get_vid();
                    (vid, pid)
                }
                None => (0, 0),
            };

            NativeEndian::write_u16(&mut buf[0..], vid);
            NativeEndian::write_u16(&mut buf[2..], pid);
            handle_eintr!(self.sock.send(&buf))
                .map(|_| ())
                .map_err(Error::Io)?;
        } else {
            panic!("error: Host device provider received unknown command");
        }
    }
}
