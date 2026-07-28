// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// MIRAI_FLAGS --diag=default

use mirai_annotations::*;
use std::sync::Arc;

struct Lock;

struct Inner {
    lock: Lock,
}

struct Owner {
    inner: Arc<Inner>,
}

impl Owner {
    fn lock(&self) -> &Lock {
        &self.inner.lock
    }

    fn invoke(&self, callback: impl FnOnce()) {
        Some(()).map(|_| callback()).unwrap();
    }

    fn require_unlocked(&self) {
        precondition!(get_model_field!(self.lock(), writer, 0usize) == 0);
    }
}

pub fn repeated_static_site_is_scoped_per_call() {
    let first = Owner {
        inner: Arc::new(Inner { lock: Lock }),
    };
    let second = Owner {
        inner: Arc::new(Inner { lock: Lock }),
    };

    set_model_field!(first.lock(), writer, 1usize);
    first.invoke(|| {});
    set_model_field!(first.lock(), writer, 0usize);
    second.invoke(|| second.require_unlocked());
}

pub fn main() {}
