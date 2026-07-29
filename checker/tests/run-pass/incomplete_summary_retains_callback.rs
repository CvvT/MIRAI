// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// MIRAI_FLAGS --diag=paranoid

use mirai_annotations::*;

fn require_nonzero(value: i32) {
    precondition!(value != 0); //~ related location
}

fn violate_precondition() {
    require_nonzero(0);
}

fn incomplete_after_callback<F: FnOnce()>(callback: F) {
    callback();
    unsafe {
        std::arch::asm!("nop"); //~ Inline assembly code cannot be analyzed
    }
}

pub fn trigger() {
    incomplete_after_callback(violate_precondition); //~ unsatisfied precondition
}

pub fn main() {}
