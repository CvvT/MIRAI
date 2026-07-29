// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// MIRAI_FLAGS --diag=paranoid

use mirai_annotations::*;

pub fn incomplete_after_verification() {
    verify!(false); //~ provably false verification condition
    unsafe {
        std::arch::asm!("nop"); //~ Inline assembly code cannot be analyzed
    }
}

pub fn main() {}
