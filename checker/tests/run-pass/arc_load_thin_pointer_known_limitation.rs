// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// This is a genuine double-lock: `acquire_write` sets the `writer` model field to 1 on the
// Arc-loaded socket-options lock, and the terminal callback calls `require_unlocked` while that
// writer is still held. Callback lineage carries the guarded model state through the nested
// `Option::map` re-wrap to the terminal callback.
//
// Positive control: model_field_wrapper_field_double_lock.rs passes the owner as an explicit
// callback argument across the same Arc and wrapper-field hop, and does report the violation.

// MIRAI_FLAGS --diag=default

use mirai_annotations::*;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

pub struct Lock;

pub struct SocketOptions;

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

    fn socket_options_mut(&self) -> SocketOptionsGuard<'_> {
        self.acquire_write();
        SocketOptionsGuard {
            owner: self,
            options: SocketOptions,
        }
    }

    fn with_socket_options_mut<R>(
        &self,
        callback: impl FnOnce(&mut SocketOptions) -> R,
    ) -> R {
        self.socket_options_mut()
            .with_socket_options_mut(|options| callback(options))
    }

    pub fn trigger(&self) {
        self.with_socket_options_mut(|_options| {
            self.require_unlocked(); //~ unsatisfied precondition
        });
    }
}

struct SocketOptionsGuard<'a> {
    owner: &'a Owner,
    options: SocketOptions,
}

impl SocketOptionsGuard<'_> {
    fn with_socket_options_mut<R>(
        &mut self,
        callback: impl FnOnce(&mut SocketOptions) -> R,
    ) -> R {
        map_socket_options_mut(&mut self.options, |options| callback(options))
    }
}

impl Deref for SocketOptionsGuard<'_> {
    type Target = SocketOptions;

    fn deref(&self) -> &SocketOptions {
        &self.options
    }
}

impl DerefMut for SocketOptionsGuard<'_> {
    fn deref_mut(&mut self) -> &mut SocketOptions {
        &mut self.options
    }
}

impl Drop for SocketOptionsGuard<'_> {
    fn drop(&mut self) {
        set_model_field!(self.owner.lock(), writer, 0usize);
    }
}

fn map_socket_options_mut<R>(
    options: &mut SocketOptions,
    callback: impl FnOnce(&mut SocketOptions) -> R,
) -> R {
    Some(options).map(|options| callback(options)).unwrap() //~ callback invocation could not be resolved
}

pub fn main() {}
