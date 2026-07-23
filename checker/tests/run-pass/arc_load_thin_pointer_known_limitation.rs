// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Known false-negative (XFAIL): Arc-load thin-pointer canonicalization.
//
// This is a genuine double-lock: `acquire_write` sets the `writer` model field to 1 on the
// descriptor lock, and the callback then calls `require_unlocked` while that writer is still
// held. A sound analysis must report the precondition violation, but MIRAI does not surface it.
//
// The lock is reached by loading through `Arc<Inner>` and returning `&self.inner.lock` from
// `lock()`. The model-field write happens behind the `acquire_write` summary boundary, so its
// receiver is that returned `&Lock`. At the consuming call site, the root is canonicalized from
// the Arc load into a fresh thin-pointer heap abstraction that is not parameter-rooted.
//
// Positive control: model_field_wrapper_field_double_lock.rs applies the write to a directly
// parameter-rooted `&Lock` across the same Arc and wrapper-field hop, and does report the violation.

// MIRAI_FLAGS --diag=default

use mirai_annotations::*;
use std::sync::Arc;

pub struct Lock;

struct Inner {
    lock: Lock,
}

pub struct Owner {
    inner: Arc<Inner>,
}

impl Owner {
    fn lock(&self) -> &Lock {
        &self.inner.lock
    }

    fn acquire_write(&self) {
        let lock = self.lock();
        set_model_field!(lock, writer, 1usize);
    }

    fn require_unlocked(&self) {
        let lock = self.lock();
        precondition!(get_model_field!(lock, writer, 0usize) == 0);
    }
}

pub struct Wrapper {
    owner: Owner,
}

impl Wrapper {
    fn with_write_held(&self, callback: impl FnOnce(&Owner)) {
        self.owner.acquire_write();
        callback(&self.owner);
    }

    pub fn trigger(&self) {
        self.with_write_held(|owner| {
            // A fixed checker should report an unsatisfied precondition here.
            owner.require_unlocked();
        });
    }
}

pub fn main() {}
