// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Positive control for the captured-upvar callback carrier. This mirrors
// callback_alias_model_field_effect.rs, except the closure reaches `state` through a by-value
// capture (an upvar) rather than an explicit callback argument. The write is held across the
// callback and the inner read violates the model-field precondition through the declared alias.
// A captured closure has a local-variable path, so it is lifted into the enclosing function's
// summary; because its capture is concretely bound to a parameter at this frame, the precondition
// is discharged here and the violation is reported, just like the argument-passed variant.

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

fn with_captured_write_held<F: FnOnce()>(state: &State, callback: F) {
    assumed_alias!(&*state.read_handle, &*state.write_handle);
    write_lock(&state.write_handle);
    callback();
}

pub fn trigger_captured(state: &State) {
    with_captured_write_held(state, || read_lock(&state.read_handle)); //~ unsatisfied precondition
    //~ unsatisfied precondition
}

pub fn main() {}
