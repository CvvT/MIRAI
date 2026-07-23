// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// MIRAI_FLAGS --diag=default

use mirai_annotations::*;
use std::sync::Arc;

pub struct Lock;

fn acquire(lock: &Lock) {
    precondition!(get_model_field!(lock, writer, 0usize) == 0); //~ related location
    set_model_field!(lock, writer, 1usize);
}

struct Two {
    a: Arc<Lock>,
    b: Arc<Lock>,
}

fn make(seed: Arc<Lock>) -> Two {
    let b = Arc::clone(&seed);
    Two { a: seed, b }
}

pub fn trigger(seed: Arc<Lock>) {
    let two = make(seed);
    acquire(&two.a);
    acquire(&two.b); //~ unsatisfied precondition
}

pub fn main() {}
