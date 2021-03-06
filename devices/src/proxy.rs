// Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Runs hardware devices in child processes.

use libc::pid_t;

use std::{self, fmt, io};
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::UnixDatagram;
use std::process;
use std::time::Duration;

use byteorder::{NativeEndian, ByteOrder};

use BusDevice;
use io_jail::{self, Minijail};

/// Errors for proxy devices.
#[derive(Debug)]
pub enum Error {
    ForkingJail(io_jail::Error),
    Io(io::Error),
}
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Error::ForkingJail(_) => write!(f, "Failed to fork jail process"),
            &Error::Io(ref e) => write!(f, "IO error configuring proxy device {}.", e),
        }
    }
}

const SOCKET_TIMEOUT_MS: u64 = 2000;
const MSG_SIZE: usize = 24;

enum Command {
    Read = 0,
    Write = 1,
    Shutdown = 2,
}

fn child_proc(sock: UnixDatagram, device: &mut BusDevice) {
    let mut running = true;

    while running {
        let mut buf = [0; MSG_SIZE];
        match handle_eintr!(sock.recv(&mut buf)) {
            Ok(c) if c != buf.len() => {
                error!("child device process incorrect recv size: got {}, expected {}",
                       c,
                       buf.len());
                break;
            }
            Err(e) => {
                error!("child device process failed recv: {}", e);
                break;
            }
            _ => {}
        }

        let cmd = NativeEndian::read_u32(&buf[0..]);
        let len = NativeEndian::read_u32(&buf[4..]) as usize;
        let offset = NativeEndian::read_u64(&buf[8..]);

        let res = if cmd == Command::Read as u32 {
            device.read(offset, &mut buf[16..16 + len]);
            handle_eintr!(sock.send(&buf))
        } else if cmd == Command::Write as u32 {
            device.write(offset, &buf[16..16 + len]);
            handle_eintr!(sock.send(&buf))
        } else if cmd == Command::Shutdown as u32 {
            running = false;
            handle_eintr!(sock.send(&buf))
        } else {
            error!("child device process unknown command: {}", cmd);
            break;
        };

        if let Err(e) = res {
            error!("error: child device process failed send: {}", e);
            break;
        }
    }
}

/// Wraps an inner `BusDevice` that is run inside a child process via fork.
///
/// Because forks are very unfriendly to destructors and all memory mappings and file descriptors
/// are inherited, this should be used as early as possible in the main process.
pub struct ProxyDevice {
    sock: UnixDatagram,
    pid: pid_t,
}

impl ProxyDevice {
    /// Takes the given device and isolates it into another process via fork before returning.
    ///
    /// The forked process will automatically be terminated when this is dropped, so be sure to keep
    /// a reference.
    ///
    /// # Arguments
    /// * `device` - The device to isolate to another process.
    /// * `keep_fds` - File descriptors that will be kept open in the child
    pub fn new<D: BusDevice>(mut device: D, jail: &Minijail, mut keep_fds: Vec<RawFd>)
            -> Result<ProxyDevice>
    {
        let (child_sock, parent_sock) = UnixDatagram::pair().map_err(Error::Io)?;

        keep_fds.push(child_sock.as_raw_fd());
        // Forking here is safe as long as the program is still single threaded.
        let pid = unsafe {
            match jail.fork(Some(&keep_fds)).map_err(Error::ForkingJail)? {
                0 => {
                    child_proc(child_sock, &mut device);
                    // ! Never returns
                    process::exit(0);
                },
                p => p,
            }
        };

        parent_sock
            .set_write_timeout(Some(Duration::from_millis(SOCKET_TIMEOUT_MS)))
            .map_err(Error::Io)?;
        parent_sock
            .set_read_timeout(Some(Duration::from_millis(SOCKET_TIMEOUT_MS)))
            .map_err(Error::Io)?;
        Ok(ProxyDevice {
               sock: parent_sock,
               pid: pid,
           })
    }

    pub fn pid(&self) -> pid_t {
        self.pid
    }

    fn send_cmd(&self, cmd: Command, offset: u64, len: u32, data: &[u8]) -> Result<()> {
        let mut buf = [0; MSG_SIZE];
        NativeEndian::write_u32(&mut buf[0..], cmd as u32);
        NativeEndian::write_u32(&mut buf[4..], len);
        NativeEndian::write_u64(&mut buf[8..], offset);
        buf[16..16 + data.len()].clone_from_slice(data);
        handle_eintr!(self.sock.send(&buf)).map(|_| ()).map_err(Error::Io)
    }

    fn recv_resp(&self, data: &mut [u8]) -> Result<()> {
        let mut buf = [0; MSG_SIZE];
        handle_eintr!(self.sock.recv(&mut buf)).map_err(Error::Io)?;
        let len = data.len();
        data.clone_from_slice(&buf[16..16 + len]);
        Ok(())
    }

    fn wait(&self) -> Result<()> {
        let mut buf = [0; MSG_SIZE];
        handle_eintr!(self.sock.recv(&mut buf)).map(|_| ()).map_err(Error::Io)
    }
}

impl BusDevice for ProxyDevice {
    fn read(&mut self, offset: u64, data: &mut [u8]) {
        let res = self.send_cmd(Command::Read, offset, data.len() as u32, &[])
            .and_then(|_| self.recv_resp(data));
        if let Err(e) = res {
            error!("failed read from child device process: {}", e);
        }
    }

    fn write(&mut self, offset: u64, data: &[u8]) {
        let res = self.send_cmd(Command::Write, offset, data.len() as u32, data)
            .and_then(|_| self.wait());
        if let Err(e) = res {
            error!("failed write to child device process: {}", e);
        }
    }
}

impl Drop for ProxyDevice {
    fn drop(&mut self) {
        let res = self.send_cmd(Command::Shutdown, 0, 0, &[]);
        if let Err(e) = res {
            error!("failed to shutdown child device process: {}", e);
        }
    }
}
