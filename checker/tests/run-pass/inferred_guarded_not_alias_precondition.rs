// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the root directory of this source tree.

// A guarded lock precondition must infer a guarded non-alias obligation. The aliased call is invalid
// only while `active == 1`; inactive and distinct-lock callers must remain accepted.

use mirai_annotations::*;

pub struct Lock;

fn require_unlocked_when(active: u32, lock: &Lock) {
    precondition!((active != 1) | (get_model_field!(lock, writer, 0usize) == 0));
    //~ related location
}

fn guarded_distinct_writers(active: u32, read: &Lock, written: &Lock) {
    set_model_field!(written, writer, 1usize);
    require_unlocked_when(active, read); //~ related location
}

pub fn active_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_distinct_writers(1, lock, lock); //~ possible alias violates precondition
}

pub fn inactive_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_distinct_writers(0, lock, lock);
}

pub fn active_distinct(read: &Lock, written: &Lock) {
    assume!(not_alias!(read, written));
    set_model_field!(read, writer, 0usize);
    guarded_distinct_writers(1, read, written);
}

pub fn main() {}
