// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#[derive(Debug)]
pub enum Error {
    /// Error happens when invoking some syscall,
    SysError(i32),
    /// Code is in a bad state, it could be illegal guest kernel operations.
    BadState,
    /// There is an unexpected bug in crosvm.
    Unknown,
}

pub type Result<T> = std::result::Result<T, Error>;

/// err_msg would be used with Result::map_err. It would print the current error, and map it if
/// needed. Using this macro because:
/// An error message is printed where the error happens, and file name, line number is included.
/// Easy to write, no need to use match just for print error.
#[macro_export]
macro_rules! err_msg {
    () => {
        |e| {
            error!("usb error: {:?}", e);
            e
        }
    };
    (Error::SysError) => {
        |e| {
            error!("usb error: {:?}", e);
            Error::SysError(e.errno())
        }
    };
    (Error::SysError, $($arg:tt)* ) => {
        |e| {
            error!("usb error: {:?}. {}", e, format!($($arg)*));
            Error::SysError(e.errno())
        }
    };
    ($err:path) => {
        |e| {
            error!("usb error: {:?}", e);
            $err
        }
    };
    ($err:path, $($arg:tt)* ) => {
        |e| {
            error!("usb error: {:?}. {}", e, format!($($arg)*));
            $err
        }
    };
    ( $($arg:tt)* ) => {
        |e| {
            error!("usb error: {:?}. {}", e, format!($($arg)*));
            e
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use sys_util::Error as SysError;
    #[test]
    fn err_msg_test() {
        sys_util::syslog::init().unwrap();
        let r: std::result::Result<(), SysError> = Err(SysError::new(123));
        let _ = r
            .map_err(err_msg!(Error::SysError))
            .map_err(err_msg!())
            .map_err(err_msg!(Error::BadState))
            .map_err(err_msg!("some info"))
            .map_err(err_msg!(
                Error::Unknown,
                "more info and a random number{}",
                3
            ));
    }
}
