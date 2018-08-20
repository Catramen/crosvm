// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::{Arc, Mutex};

/// AutoCallback wraps a callback. The callback will be invoked when last instance of AutoCallback
/// and its clones is dropped.
#[derive(Clone)]
pub struct AutoCallback {
    inner: Arc<Mutex<AutoCallbackInner>>,
}

impl AutoCallback {
    /// Create new callback from closure.
    pub fn new<C: 'static + FnMut() + Send>(cb: C) -> AutoCallback {
        AutoCallback {
            inner: Arc::new(Mutex::new(AutoCallbackInner {
                callback: Box::new(cb),
            })),
        }
    }
}

struct AutoCallbackInner {
    callback: Box<FnMut() + Send>,
}

impl Drop for AutoCallbackInner {
    fn drop(&mut self) {
        (self.callback)();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn task(_: AutoCallback) {}

    #[test]
    fn simple_raii_callback() {
        let a = Arc::new(Mutex::new(0));
        let ac = a.clone();
        let cb = AutoCallback::new(move || {
            *ac.lock().unwrap() = 1;
        });
        task(cb.clone());
        task(cb.clone());
        task(cb);
        assert_eq!(*a.lock().unwrap(), 1);
    }
}
