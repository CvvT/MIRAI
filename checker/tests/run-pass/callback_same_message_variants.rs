// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use mirai_annotations::*;

fn require_nonzero(value: i32) {
    precondition!(value != 0);
}

fn invoke<F: FnOnce()>(callback: F) {
    callback();
}

fn require_via_callbacks(value: i32, use_first: bool) {
    if use_first {
        invoke(|| require_nonzero(value)); //~ related location
    } else {
        invoke(|| require_nonzero(value));
    }
}

pub fn trigger() {
    require_via_callbacks(0, true); //~ unsatisfied precondition
}

pub fn main() {}
