// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use mirai_annotations::*;

pub struct Lock;

fn write_lock(lock: &Lock) {
    set_model_field!(lock, writer, 1usize);
}

fn read_lock(lock: &Lock) {
    precondition!(get_model_field!(lock, writer, 0usize) == 0);
}

fn replace_before_callback<'a, F: FnOnce(&'a Lock)>(
    mut current: &'a Lock,
    replacement: &'a Lock,
    callback: F,
) {
    write_lock(current);
    current = replacement;
    callback(current);
}

pub fn trigger(current: &Lock, replacement: &Lock) {
    assume!(get_model_field!(replacement, writer, 0usize) == 0);
    replace_before_callback(current, replacement, read_lock);
}

pub fn main() {}
