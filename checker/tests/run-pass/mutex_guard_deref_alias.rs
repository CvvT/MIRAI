// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Positive control for the MutexGuard deref-identity model. The model subject is reached through a
// std `Mutex::lock()` -> `MutexGuard` -> `Deref` on both the write side and the read side.

// MIRAI_FLAGS --diag=default

use mirai_annotations::*;
use std::sync::{Arc, Mutex};

pub struct Lock;

fn write_lock(lock: &Lock) {
    precondition!(get_model_field!(lock, writer, 0usize) == 0); //~ related location
    set_model_field!(lock, writer, 1usize);
}

fn read_lock(lock: &Lock) {
    precondition!(get_model_field!(lock, writer, 0usize) == 0);
}

pub struct Inner {
    litebox: Arc<Lock>,
}

pub struct State {
    net: Mutex<Inner>,
    litebox: Arc<Lock>,
}

pub fn trigger_deep(state: &State) {
    {
        let g = state.net.lock().unwrap();
        assumed_alias!(&*g.litebox, &*state.litebox);
        write_lock(&state.litebox); //~ related location
    }

    (|| {
        //~ unsatisfied precondition
        let g = state.net.lock().unwrap();
        read_lock(&g.litebox);
    })();
}

pub struct DistinctState {
    first: Mutex<Inner>,
    second: Mutex<Inner>,
    first_litebox: Arc<Lock>,
}

pub fn distinct_mutexes_do_not_alias(state: &DistinctState) {
    {
        let first = state.first.lock().unwrap();
        assumed_alias!(&*first.litebox, &*state.first_litebox);
        write_lock(&state.first_litebox);
    }

    let second = state.second.lock().unwrap();
    read_lock(&second.litebox);
}

pub fn main() {}
