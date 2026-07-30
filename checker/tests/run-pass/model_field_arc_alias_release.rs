// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Path-B discharge reduction: a constructor (`make`) derives an Arc alias purely via
// `Arc::clone` into two struct fields (no `assumed_alias!` annotation), and separate
// whole-program roots exercise the aliased model field. The buggy root acquires the
// writer on one handle and then reads through the *other* handle without releasing, so
// the aliased `writer` model field is still 1 and the read precondition must fire. The
// fixed root releases before the aliased read, so the propagated `writer == 0` must make
// the read clean. This isolates exactly what the LiteBox whole-program discharge must
// prove — that release state canonicalizes across a constructor-derived Arc alias — with
// no annotation and no smoltcp constructor barrier in the way.

// MIRAI_FLAGS --diag=default

use mirai_annotations::*;
use std::sync::Arc;

pub struct Lock;

fn acquire(lock: &Lock) {
    precondition!(get_model_field!(lock, writer, 0usize) == 0);
    set_model_field!(lock, writer, 1usize);
}

fn release(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
}

fn read(lock: &Lock) {
    precondition!(get_model_field!(lock, writer, 0usize) == 0); //~ related location
}

struct Two {
    a: Arc<Lock>,
    b: Arc<Lock>,
}

fn make(seed: Arc<Lock>) -> Two {
    let b = Arc::clone(&seed);
    Two { a: seed, b }
}

pub fn trigger_buggy(seed: Arc<Lock>) {
    let two = make(seed);
    acquire(&two.a);
    read(&two.b); //~ unsatisfied precondition
}

pub fn trigger_fixed(seed: Arc<Lock>) {
    let two = make(seed);
    acquire(&two.a);
    release(&two.a);
    read(&two.b);
}

pub fn main() {}
