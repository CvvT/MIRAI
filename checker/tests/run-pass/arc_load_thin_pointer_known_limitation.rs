// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Known false-negative (XFAIL): Arc-load thin-pointer canonicalization loses the lock's precise
// model-field identity across the returned-guard callback summary, so default diagnostics are silent.

// MIRAI_FLAGS --diag=default

use mirai_annotations::*;
use std::ops::Deref;
use std::sync::Arc;

struct Lock<T> {
    data: T,
}

impl<T> Lock<T> {
    fn write(&self) -> WriteGuard<'_, T> {
        precondition!(get_model_field!(self, writer, 0usize) == 0);
        set_model_field!(self, writer, 1usize);
        WriteGuard { lock: self }
    }
}

struct WriteGuard<'a, T> {
    lock: &'a Lock<T>,
}

impl<T> Deref for WriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.lock.data
    }
}

struct Data;

impl Data {
    fn with_ref(&self, callback: impl FnOnce(&Data)) {
        callback(self);
    }
}

struct Inner {
    lock: Lock<Data>,
}

pub struct Owner {
    inner: Arc<Inner>,
}

impl Owner {
    fn lock(&self) -> &Lock<Data> {
        &self.inner.lock
    }

    fn data(&self) -> impl Deref<Target = Data> + '_ {
        self.lock().write()
    }

    fn with_data(&self, callback: impl FnOnce(&Data)) {
        self.data().with_ref(callback);
    }

    pub fn trigger(&self) {
        self.with_data(|_data| {
            // A fixed checker should report an unsatisfied precondition here.
            precondition!(get_model_field!(self.lock(), writer, 0usize) == 0);
        });
    }
}

pub fn main() {}
