// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// MIRAI_FLAGS --diag=default

use mirai_annotations::*;
use std::sync::Arc;

pub struct Lock;

fn acquire(lock: &Lock) {
    precondition!(get_model_field!(lock, writer, 0usize) == 0);
    set_model_field!(lock, writer, 1usize);
}

pub fn independent() {
    let first = Arc::new(Lock);
    let second = Arc::new(Lock);
    acquire(&first);
    acquire(&second);
}

pub fn main() {}
