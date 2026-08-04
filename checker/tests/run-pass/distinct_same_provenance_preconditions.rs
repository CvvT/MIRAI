// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use mirai_annotations::*;

fn require_nonzero(value: i32) {
    precondition!(value != 0); //~ related location
}

fn invoke<F: FnOnce()>(callback: F) {
    callback();
}

fn require_both_nonzero(first: i32, second: i32) {
    invoke(|| {
        require_nonzero(first);
        require_nonzero(second);
    });
}

pub fn trigger() {
    require_both_nonzero(1, 0); //~ unsatisfied precondition
}

pub fn main() {}
