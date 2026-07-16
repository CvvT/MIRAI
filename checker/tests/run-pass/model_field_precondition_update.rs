// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use mirai_annotations::*;

struct Lock;

impl Lock {
    fn acquire_write(&self) {
        precondition!(get_model_field!(self, writer, 0usize) == 0); //~ related location
        set_model_field!(self, writer, 1usize);
    }

    fn acquire_read(&self) {
        let readers = get_model_field!(self, readers, 0usize);
        precondition!(readers < usize::MAX);
        set_model_field!(self, readers, readers + 1);
    }
}

pub fn double_write_is_reachable() {
    let lock = Lock;
    lock.acquire_write();
    verify!(false); //~ provably false verification condition
    lock.acquire_write(); //~ unsatisfied precondition
}

pub fn read_continuation_stays_reachable() {
    let lock = Lock;
    lock.acquire_read();
    verify!(false); //~ provably false verification condition
}

fn positive_identity(value: i32) -> i32 {
    precondition!(value > 0);
    value
}

pub fn scalar_precondition_still_refines() {
    let value = positive_identity(1);
    verify!(value == 1);
}

pub fn main() {}
