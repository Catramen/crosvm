// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use bindings::*;

const U: u32 = 'U' as u32;

ioctl_iowr_nr!(USBDEVFS_CONTROL, U, 0, usbdevfs_ctrltransfer);
ioctl_iowr_nr!(USBDEVFS_BULK, U, 2, usbdevfs_bulktransfer);
ioctl_ior_nr!(USBDEVFS_RESETEP, U, 3, ::std::os::raw::c_uint);
ioctl_ior_nr!(USBDEVFS_SETINTERFACE, U, 4, usbdevfs_setinterface);
ioctl_ior_nr!(USBDEVFS_SETCONFIGURATION, U, 5, ::std::os::raw::c_uint);
ioctl_iow_nr!(USBDEVFS_GETDRIVER, U, 8, usbdevfs_getdriver);
ioctl_ior_nr!(USBDEVFS_SUBMITURB, U, 10, usbdevfs_urb);
ioctl_io_nr!(USBDEVFS_DISCARDURB, U, 11);
ioctl_iow_nr!(USBDEVFS_REAPURB, U, 12, *mut ::std::os::raw::c_void);
ioctl_iow_nr!(USBDEVFS_REAPURBNDELAY, U, 13, *mut ::std::os::raw::c_void);
ioctl_ior_nr!(USBDEVFS_DISCSIGNAL, U, 14, usbdevfs_disconnectsignal);
ioctl_ior_nr!(USBDEVFS_CLAIMINTERFACE, U, 15, ::std::os::raw::c_uint);
ioctl_ior_nr!(USBDEVFS_RELEASEINTERFACE, U, 16, ::std::os::raw::c_uint);
ioctl_iow_nr!(USBDEVFS_CONNECTINF, U, 17, usbdevfs_connectinfo);
ioctl_iowr_nr!(USBDEVFS_IOCTL, U, 18, usbdevfs_ioctl);
ioctl_ior_nr!(USBDEVFS_HUB_PORTINFO, U, 19, usbdevfs_hub_portinfo);
ioctl_io_nr!(USBDEVFS_RESET, U, 20);
ioctl_ior_nr!(USBDEVFS_CLEAR_HALT, U, 21, ::std::os::raw::c_uint);
ioctl_io_nr!(USBDEVFS_DISCONNECT, U, 22);
ioctl_io_nr!(USBDEVFS_CONNECT, U, 23);
ioctl_ior_nr!(USBDEVFS_CLAIM_PORT, U, 24, ::std::os::raw::c_uint);
ioctl_ior_nr!(USBDEVFS_RELEASE_PORT, U, 25, ::std::os::raw::c_uint);
ioctl_ior_nr!(USBDEVFS_DISCONNECT_CLAIM, U, 27, usbdevfs_disconnect_claim);
ioctl_ior_nr!(USBDEVFS_ALLOC_STREAMS, U, 28, usbdevfs_streams);
ioctl_ior_nr!(USBDEVFS_FREE_STREAMS, U, 29, usbdevfs_streams);
