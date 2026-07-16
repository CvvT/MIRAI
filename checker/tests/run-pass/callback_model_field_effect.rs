// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use mirai_annotations::*;

struct Lock;

fn acquire_write(lock: &Lock) {
    precondition!(get_model_field!(lock, writer, 0usize) == 0); //~ related location
    set_model_field!(lock, writer, 1usize);
}

fn invoke_twice<F>(lock: &Lock, callback: F)
where
    F: Fn(&Lock),
{
    callback(lock);
    callback(lock); //~ unsatisfied precondition
}

pub fn main() {
    let lock = Lock;
    invoke_twice(&lock, acquire_write);
}
