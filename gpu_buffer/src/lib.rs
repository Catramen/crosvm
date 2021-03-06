// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! A crate for creating [DRM](https://en.wikipedia.org/wiki/Direct_Rendering_Manager) managed
//! buffer objects. Such objects are useful for exporting as DMABUFs/prime FDs, texturing, render
//! targets, memory mapping, and scanout.
//!
//! # Examples
//!
//! ```rust
//! # use std::error::Error;
//! # use std::fs::File;
//! # use std::result::Result;
//! # use gpu_buffer::*;
//! # fn test() -> Result<(), Box<Error>> {
//! let drm_card = File::open("/dev/dri/card0")?;
//! let device = Device::new(drm_card).map_err(|_| "failed to create device")?;
//! let bo = device
//!     .create_buffer(1024,
//!                    512,
//!                    Format::new(b'X', b'R', b'2', b'4'),
//!                    Flags::empty().use_scanout(true))
//!     .map_err(|_| "failed to create buffer")?;
//! assert_eq!(bo.width(), 1024);
//! assert_eq!(bo.height(), 512);
//! assert_eq!(bo.format(), Format::new(b'X', b'R', b'2', b'4'));
//! assert_eq!(bo.num_planes(), 1);
//! # Ok(())
//! # }
//! ```

extern crate data_model;
#[macro_use]
extern crate sys_util;

pub mod rendernode;
mod raw;

use std::os::raw::c_void;
use std::fmt;
use std::cmp::min;
use std::fs::File;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::ptr::{copy_nonoverlapping, null_mut};
use std::rc::Rc;
use std::result::Result;

use data_model::VolatileSlice;

use raw::*;

const MAP_FAILED: *mut c_void = (-1isize as *mut _);

/// A [fourcc](https://en.wikipedia.org/wiki/FourCC) format identifier.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Format(u32);

impl Format {
    /// Constructs a format identifer using a fourcc byte sequence.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gpu_buffer::Format;
    ///
    /// let format = Format::new(b'X', b'R', b'2', b'4');
    /// println!("format: {:?}", format);
    /// ```
    #[inline(always)]
    pub fn new(a: u8, b: u8, c: u8, d: u8) -> Format {
        Format(a as u32 | (b as u32) << 8 | (c as u32) << 16 | (d as u32) << 24)
    }

    /// Returns the fourcc code as a sequence of bytes.
    #[inline(always)]
    pub fn to_bytes(&self) -> [u8; 4] {
        let f = self.0;
        [f as u8, (f >> 8) as u8, (f >> 16) as u8, (f >> 24) as u8]
    }
}

impl From<u32> for Format {
    fn from(u: u32) -> Format {
        Format(u)
    }
}

impl From<Format> for u32 {
    fn from(f: Format) -> u32 {
        f.0
    }
}

impl fmt::Debug for Format {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let b = self.to_bytes();
        if b.iter().all(u8::is_ascii_graphic) {
            write!(f,
                   "fourcc({}{}{}{})",
                   b[0] as char,
                   b[1] as char,
                   b[2] as char,
                   b[3] as char)
        } else {
            write!(f,
                   "fourcc(0x{:02x}{:02x}{:02x}{:02x})",
                   b[0],
                   b[1],
                   b[2],
                   b[3])
        }
    }
}

/// Usage flags for constructing a buffer object.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Flags(u32);

impl Flags {
    /// Returns empty set of flags.
    #[inline(always)]
    pub fn empty() -> Flags {
        Flags(0)
    }

    /// Returns the given set of raw `GBM_BO` flags wrapped in a `Flags` struct.
    #[inline(always)]
    pub fn new(raw: u32) -> Flags {
        Flags(raw)
    }

    /// Sets the scanout flag's presence
    #[inline(always)]
    pub fn use_scanout(self, e: bool) -> Flags {
        if e {
            Flags(self.0 | GBM_BO_USE_SCANOUT)
        } else {
            Flags(self.0 & !GBM_BO_USE_SCANOUT)
        }
    }

    /// Sets the cursor flag's presence
    #[inline(always)]
    pub fn use_cursor(self, e: bool) -> Flags {
        if e {
            Flags(self.0 | GBM_BO_USE_CURSOR)
        } else {
            Flags(self.0 & !GBM_BO_USE_CURSOR)
        }
    }

    /// Sets the cursor 64x64 flag's presence
    #[inline(always)]
    pub fn use_cursor64(self, e: bool) -> Flags {
        if e {
            Flags(self.0 | GBM_BO_USE_CURSOR_64X64)
        } else {
            Flags(self.0 & !GBM_BO_USE_CURSOR_64X64)
        }
    }

    /// Sets the rendering flag's presence
    #[inline(always)]
    pub fn use_rendering(self, e: bool) -> Flags {
        if e {
            Flags(self.0 | GBM_BO_USE_RENDERING)
        } else {
            Flags(self.0 & !GBM_BO_USE_RENDERING)
        }
    }

    /// Sets the linear flag's presence
    #[inline(always)]
    pub fn use_linear(self, e: bool) -> Flags {
        if e {
            Flags(self.0 | GBM_BO_USE_LINEAR)
        } else {
            Flags(self.0 & !GBM_BO_USE_LINEAR)
        }
    }

    /// Sets the texturing flag's presence
    #[inline(always)]
    pub fn use_texturing(self, e: bool) -> Flags {
        if e {
            Flags(self.0 | GBM_BO_USE_TEXTURING)
        } else {
            Flags(self.0 & !GBM_BO_USE_TEXTURING)
        }
    }
}


struct DeviceInner {
    _fd: File,
    gbm: *mut gbm_device,
}

impl Drop for DeviceInner {
    fn drop(self: &mut DeviceInner) {
        // Safe because DeviceInner is only constructed with a valid gbm_device.
        unsafe {
            gbm_device_destroy(self.gbm);
        }
    }
}

/// A device capable of allocating `Buffer`.
#[derive(Clone)]
pub struct Device(Rc<DeviceInner>);

impl Device {
    /// Returns a new `Device` using the given `fd` opened from a device in `/dev/dri/`.
    pub fn new(fd: File) -> Result<Device, ()> {
        // gbm_create_device is safe to call with a valid fd, and we check that a valid one is
        // returned. The FD is not of the appropriate kind (i.e. not a DRM device),
        // gbm_create_device should reject it.
        let gbm = unsafe { gbm_create_device(fd.as_raw_fd()) };
        if gbm.is_null() {
            Err(())
        } else {
            Ok(Device(Rc::new(DeviceInner { _fd: fd, gbm })))
        }
    }

    /// Creates a new buffer with the given metadata.
    pub fn create_buffer(&self,
                         width: u32,
                         height: u32,
                         format: Format,
                         usage: Flags)
                         -> Result<Buffer, ()> {
        // This is safe because only a valid gbm_device is used and the return value is checked.
        let bo = unsafe { gbm_bo_create(self.0.gbm, width, height, format.0, usage.0) };
        if bo.is_null() {
            Err(())
        } else {
            Ok(Buffer(bo, self.clone()))
        }
    }
}

/// An allocation from a `Device`.
pub struct Buffer(*mut gbm_bo, Device);

impl Buffer {
    /// The device
    pub fn device(&self) -> &Device {
        &self.1
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        // This is always safe to call with a valid gbm_bo pointer.
        unsafe { gbm_bo_get_width(self.0) }
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        // This is always safe to call with a valid gbm_bo pointer.
        unsafe { gbm_bo_get_height(self.0) }
    }

    /// Length in bytes of one row of the buffer.
    pub fn stride(&self) -> u32 {
        // This is always safe to call with a valid gbm_bo pointer.
        unsafe { gbm_bo_get_stride(self.0) }
    }

    /// Length in bytes of the stride or tiling.
    pub fn stride_or_tiling(&self) -> u32 {
        // This is always safe to call with a valid gbm_bo pointer.
        unsafe { gbm_bo_get_stride_or_tiling(self.0) }
    }

    /// `Format` of the buffer.
    pub fn format(&self) -> Format {
        // This is always safe to call with a valid gbm_bo pointer.
        unsafe { Format(gbm_bo_get_format(self.0)) }
    }

    /// Format modifier flags for the buffer.
    pub fn format_modifier(&self) -> u64 {
        // This is always safe to call with a valid gbm_bo pointer.
        unsafe { gbm_bo_get_format_modifier(self.0) }
    }

    /// Number of planes present in this buffer.
    pub fn num_planes(&self) -> usize {
        // This is always safe to call with a valid gbm_bo pointer.
        unsafe { gbm_bo_get_num_planes(self.0) }
    }

    /// Exports a new dmabuf/prime file descriptor for the given plane.
    pub fn export_plane_fd(&self, plane: usize) -> Result<File, i32> {
        // This is always safe to call with a valid gbm_bo pointer.
        match unsafe { gbm_bo_get_plane_fd(self.0, plane) } {
            fd if fd >= 0 => Ok(unsafe { File::from_raw_fd(fd) }),
            ret => Err(ret),
        }
    }

    /// Reads the given subsection of the buffer to `dst`.
    pub fn read_to_volatile(&self,
                            x: u32,
                            y: u32,
                            width: u32,
                            height: u32,
                            plane: usize,
                            dst: VolatileSlice)
                            -> Result<(), ()> {
        let mut stride = 0;
        let mut map_data = null_mut();
        // Safe because only a valid gbm_bo object is used and the return value is checked. Only
        // pointers coerced from stack references are used for returned values, and we trust gbm to
        // only write as many bytes as the size of the pointed to values.
        let mapping = unsafe {
            gbm_bo_map(self.0,
                       x,
                       y,
                       width,
                       height,
                       GBM_BO_TRANSFER_READ,
                       &mut stride,
                       &mut map_data,
                       plane)
        };
        if mapping == MAP_FAILED {
            return Err(());
        }

        let copy_size = (y as u64) * (stride as u64);

        let res = if copy_size <= dst.size() {
            // The two buffers can not be overlapping because we just made a new mapping in this
            // scope.
            unsafe {
                copy_nonoverlapping(mapping as *mut u8, dst.as_ptr(), copy_size as usize);
            }
            Ok(())
        } else {
            Err(())
        };

        // safe because the gbm_bo is assumed to be valid and the map_data is the same one given by
        // gbm_bo_map.
        unsafe {
            gbm_bo_unmap(self.0, map_data);
        }

        res
    }

    /// Writes to the given subsection of the buffer from `src`.
    pub fn write_from_slice(&self,
                            x: u32,
                            y: u32,
                            width: u32,
                            height: u32,
                            plane: usize,
                            src: &[u8])
                            -> Result<(), ()> {
        let mut stride = 0;
        let mut map_data = null_mut();
        // Safe because only a valid gbm_bo object is used and the return value is checked. Only
        // pointers coerced from stack references are used for returned values, and we trust gbm to
        // only write as many bytes as the size of the pointed to values.
        let mapping = unsafe {
            gbm_bo_map(self.0,
                       x,
                       y,
                       width,
                       height,
                       GBM_BO_TRANSFER_WRITE,
                       &mut stride,
                       &mut map_data,
                       plane)
        };
        if mapping == MAP_FAILED {
            return Err(());
        }

        let copy_size = (height as u64) * (stride as u64);
        let copy_sg_size = min(src.len() as u64, copy_size);

        // The two buffers can not be overlapping because we just made a new mapping in this scope.
        unsafe {
            copy_nonoverlapping(src.as_ptr(), mapping as *mut u8, copy_sg_size as usize);
        }

        // safe because the gbm_bo is assumed to be valid and the map_data is the same one given by
        // gbm_bo_map.
        unsafe {
            gbm_bo_unmap(self.0, map_data);
        }

        Ok(())
    }

    /// Writes to the given subsection of the buffer from `sgs`.
    pub fn write_from_sg<'a, S: Iterator<Item = VolatileSlice<'a>>>(&self,
                                                                    x: u32,
                                                                    y: u32,
                                                                    width: u32,
                                                                    height: u32,
                                                                    plane: usize,
                                                                    sgs: S)
                                                                    -> Result<(), ()> {
        let mut stride = 0;
        let mut map_data = null_mut();
        // Safe because only a valid gbm_bo object is used and the return value is checked. Only
        // pointers coerced from stack references are used for returned values, and we trust gbm to
        // only write as many bytes as the size of the pointed to values.
        let mut mapping = unsafe {
            gbm_bo_map(self.0,
                       x,
                       y,
                       width,
                       height,
                       GBM_BO_TRANSFER_WRITE,
                       &mut stride,
                       &mut map_data,
                       plane)
        };
        if mapping == MAP_FAILED {
            return Err(());
        }

        let mut copy_size = (height as u64) * (stride as u64);

        for sg in sgs {
            let copy_sg_size = min(sg.size(), copy_size);
            // The two buffers can not be overlapping because we just made a new mapping in this
            // scope.
            unsafe {
                copy_nonoverlapping(sg.as_ptr(), mapping as *mut u8, copy_sg_size as usize);
            }

            mapping = mapping.wrapping_offset(copy_sg_size as isize);
            copy_size -= copy_sg_size;
            if copy_size == 0 {
                break;
            }
        }

        // safe because the gbm_bo is assumed to be valid and the map_data is the same one given by
        // gbm_bo_map.
        unsafe {
            gbm_bo_unmap(self.0, map_data);
        }

        Ok(())
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        // This is always safe to call with a valid gbm_bo pointer.
        unsafe { gbm_bo_destroy(self.0) }
    }
}

impl AsRawFd for Buffer {
    fn as_raw_fd(&self) -> RawFd {
        // This is always safe to call with a valid gbm_bo pointer.
        unsafe { gbm_bo_get_fd(self.0) }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write;
    use data_model::VolatileMemory;

    #[test]
    fn format_debug() {
        let f = Format::new(b'X', b'R', b'2', b'4');
        let mut buf = String::new();
        write!(&mut buf, "{:?}", f).unwrap();
        assert_eq!(buf, "fourcc(XR24)");

        let f = Format::new(0, 1, 2, 16);
        let mut buf = String::new();
        write!(&mut buf, "{:?}", f).unwrap();
        assert_eq!(buf, "fourcc(0x00010210)");
    }

    #[test]
    #[ignore] // no access to /dev/dri
    fn open_device() {
        let drm_card = File::open("/dev/dri/card0").expect("failed to open card");
        Device::new(drm_card).expect("failed to create device with card");
    }

    #[test]
    #[ignore] // no access to /dev/dri
    fn create_buffer() {
        let drm_card = File::open("/dev/dri/card0").expect("failed to open card");
        let device = Device::new(drm_card).expect("failed to create device with card");
        let bo = device
            .create_buffer(1024,
                           512,
                           Format::new(b'X', b'R', b'2', b'4'),
                           Flags::empty().use_scanout(true))
            .expect("failed to create buffer");

        assert_eq!(bo.width(), 1024);
        assert_eq!(bo.height(), 512);
        assert_eq!(bo.format(), Format::new(b'X', b'R', b'2', b'4'));
        assert_eq!(bo.num_planes(), 1);
    }

    #[test]
    #[ignore] // no access to /dev/dri
    fn export_buffer() {
        let drm_card = File::open("/dev/dri/card0").expect("failed to open card");
        let device = Device::new(drm_card).expect("failed to create device with card");
        let bo = device
            .create_buffer(1024,
                           1024,
                           Format::new(b'X', b'R', b'2', b'4'),
                           Flags::empty().use_scanout(true))
            .expect("failed to create buffer");
        bo.export_plane_fd(0).expect("failed to export plane");
    }


    #[test]
    #[ignore] // no access to /dev/dri
    fn buffer_transfer() {
        let drm_card = File::open("/dev/dri/card0").expect("failed to open card");
        let device = Device::new(drm_card).expect("failed to create device with card");
        let bo = device
            .create_buffer(1024,
                           1024,
                           Format::new(b'X', b'R', b'2', b'4'),
                           Flags::empty().use_scanout(true).use_linear(true))
            .expect("failed to create buffer");
        let mut dst: Vec<u8> = Vec::new();
        dst.resize((bo.stride() * bo.height()) as usize, 0x4A);
        let dst_len = dst.len() as u64;
        bo.write_from_slice(0, 0, 1024, 1024, 0, dst.as_slice())
            .expect("failed to read bo");
        bo.read_to_volatile(0,
                              0,
                              1024,
                              1024,
                              0,
                              dst.as_mut_slice().get_slice(0, dst_len).unwrap())
            .expect("failed to read bo");
        assert!(dst.iter().all(|&x| x == 0x4A));
    }
}
