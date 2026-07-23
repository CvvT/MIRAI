// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use mirai_annotations::*;
use std::sync::Arc;

pub struct Lock;

fn write_lock(lock: &Lock) {
    precondition!(get_model_field!(lock, writer, 0usize) == 0);
    set_model_field!(lock, writer, 1usize);
}

fn read_lock(lock: &Lock) {
    precondition!(get_model_field!(lock, writer, 0usize) == 0); //~ related location
    //~ related location
}

pub struct State {
    write_handle: Arc<Lock>,
    read_handle: Arc<Lock>,
}

fn with_write_held<F: FnOnce(&State)>(state: &State, callback: F) {
    assumed_alias!(&*state.read_handle, &*state.write_handle);
    write_lock(&state.write_handle);
    callback(state);
}

pub fn trigger(state: &State) {
    with_write_held(state, |state| read_lock(&state.read_handle)); //~ unsatisfied precondition
    //~ unsatisfied precondition
}

pub fn main() {}
