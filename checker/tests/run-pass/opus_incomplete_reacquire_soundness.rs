// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// Adversarial soundness probe for the incomplete_model_state cleanup channel.
// A wrapper releases the writer, invokes an unresolvable/incomplete callback, and
// then RE-ACQUIRES the writer after the incomplete point. If the cleanup channel
// unsoundly exports the intermediate `writer = 0` (ignoring the later reacquire),
// the caller's require_unlocked would wrongly pass. A sound analyzer must fire.

use mirai_annotations::*;

pub struct Lock;

fn require_unlocked(lock: &Lock) {
    precondition!(get_model_field!(lock, writer, 0usize) == 0); //~ related location
}

fn opaque() {
    unsafe {
        std::arch::asm!("nop"); //~ Inline assembly code cannot be analyzed
    }
}

fn release_then_incomplete_then_reacquire<F: FnOnce()>(lock: &Lock, callback: F) {
    set_model_field!(lock, writer, 0usize);
    callback();
    set_model_field!(lock, writer, 1usize);
}

pub fn trigger(lock: &Lock) {
    set_model_field!(lock, writer, 1usize);
    release_then_incomplete_then_reacquire(lock, opaque);
    require_unlocked(lock); //~ unsatisfied precondition
}

pub fn main() {}
