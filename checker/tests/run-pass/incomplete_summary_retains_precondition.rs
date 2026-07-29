// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// MIRAI_FLAGS --diag=default

use mirai_annotations::*;

fn require_nonzero(value: i32) {
    precondition!(value != 0); //~ related location
    //~ related location
}

fn incomplete_after_precondition(value: i32) {
    require_nonzero(value);
    unsafe {
        std::arch::asm!("nop"); //~ Inline assembly code cannot be analyzed
    }
}

pub fn trigger() {
    incomplete_after_precondition(0); //~ unsatisfied precondition
}

pub fn main() {}
