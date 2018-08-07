// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::Arc;

/// AutoCallback wraps a callback. The callback will be invoked when last instance of AutoCallback
/// and it's clones is dropped.
#[derive(Clone)]
pub struct AutoCallback {
    inner: Arc<AutoCallbackInner>,
}

impl AutoCallback {
    pub fn new<C: 'static + Fn()>(cb: C) -> AutoCallback {
        AutoCallback {
            inner: Arc::new(AutoCallbackInner {
                callback: Box::new(cb),
            }),
        }
    }
}

struct AutoCallbackInner {
    callback: Box<Fn()>,
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
