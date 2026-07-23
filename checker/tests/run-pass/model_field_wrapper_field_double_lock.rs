// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Ensures Track A model-field normalization survives a wrapper-field hop.

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
}

fn acquire(lock: &Lock) {
    set_model_field!(lock, writer, 1usize);
}

fn require_unlocked(lock: &Lock) {
    precondition!(get_model_field!(lock, writer, 0usize) == 0);
}

pub struct Wrapper {
    owner: Owner,
}

impl Wrapper {
    fn with_write_held(&self, callback: impl FnOnce(&Owner)) {
        acquire(self.owner.lock());
        callback(&self.owner);
    }

    pub fn trigger(&self) {
        self.with_write_held(|owner| {
            require_unlocked(owner.lock()); //~ unsatisfied precondition
        });
    }
}

pub fn main() {}
