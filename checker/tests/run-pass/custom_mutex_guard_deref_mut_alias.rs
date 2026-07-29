// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Regression for custom guards whose lock and DerefMut implementations are analyzed from MIR
// summaries. The checker must follow the transferred pointer chain rather than assuming a standard
// library Mutex layout.

// MIRAI_FLAGS --diag=default

use mirai_annotations::*;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

pub struct Mutex<T> {
    data: T,
}

impl<T> Mutex<T> {
    pub fn lock(&mut self) -> MutexGuard<'_, T> {
        MutexGuard {
            data: &mut self.data,
        }
    }
}

pub struct MutexGuard<'a, T> {
    data: &'a mut T,
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.data
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.data
    }
}

pub struct Lock;

fn write_lock(lock: &Lock) {
    set_model_field!(lock, writer, 1usize);
}

fn read_lock(lock: &Lock) {
    precondition!(get_model_field!(lock, writer, 0usize) == 0); //~ related location
}

pub struct Inner {
    litebox: Arc<Lock>,
}

impl Inner {
    fn assume_litebox_alias(&self, source: &Arc<Lock>) {
        assumed_alias!(&*self.litebox, &**source);
    }
}

pub struct State {
    net: Mutex<Inner>,
    litebox: Arc<Lock>,
}

pub fn custom_deref_mut_replays_caller_visible_alias(state: &mut State) {
    state.net.lock().assume_litebox_alias(&state.litebox);
    write_lock(&state.litebox);

    (|| {
        //~ unsatisfied precondition
        let mut guard = state.net.lock();
        let inner: &mut Inner = &mut guard;
        //~ related location
        read_lock(&inner.litebox);
    })();
}

pub struct DistinctState {
    net: Mutex<Inner>,
    litebox: Arc<Lock>,
}

pub fn custom_guard_without_alias_stays_inert(state: &mut DistinctState) {
    write_lock(&state.litebox);

    let mut guard = state.net.lock();
    let inner: &mut Inner = &mut guard;
    read_lock(&inner.litebox);
}

pub fn main() {}
