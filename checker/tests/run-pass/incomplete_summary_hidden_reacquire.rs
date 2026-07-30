// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use mirai_annotations::*;

pub struct Lock;

fn require_unlocked(lock: &Lock) {
    precondition!(get_model_field!(lock, writer, 0usize) == 0); //~ related location
}

fn incomplete_reacquire(lock: &Lock) {
    unsafe {
        std::arch::asm!("nop"); //~ Inline assembly code cannot be analyzed
    }
    set_model_field!(lock, writer, 1usize);
}

fn release_then_incomplete_reacquire(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    incomplete_reacquire(lock);
}

pub fn trigger(lock: &Lock) {
    set_model_field!(lock, writer, 1usize);
    release_then_incomplete_reacquire(lock);
    require_unlocked(lock); //~ unsatisfied precondition
}

pub fn main() {}
