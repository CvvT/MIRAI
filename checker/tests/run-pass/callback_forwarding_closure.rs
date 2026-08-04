// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use mirai_annotations::*;
use std::sync::Arc;

pub struct State {
    lock: Arc<()>,
}

fn require_nonzero() {
    precondition!(false); //~ related location
    //~ related location
}

fn invoke<F: FnOnce()>(callback: F) {
    callback();
}

fn forward<F: FnOnce()>(state: &State, callback: F) {
    set_model_field!(&*state.lock, marker, 1usize);
    invoke(|| callback()); //~ unsatisfied precondition
}

pub fn trigger(state: &State) {
    forward(state, require_nonzero); //~ unsatisfied precondition
}

pub fn main() {}
