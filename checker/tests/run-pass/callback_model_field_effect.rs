// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use mirai_annotations::*;
use std::sync::Arc;

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

pub struct ArcLock {
    lock: Arc<Lock>,
}

fn acquire_arc_write(lock: &ArcLock) {
    precondition!(get_model_field!(&*lock.lock, writer, 0usize) == 0); //~ related location
    set_model_field!(&*lock.lock, writer, 1usize);
}

fn set_arc_writer(lock: &ArcLock, value: usize) {
    set_model_field!(&*lock.lock, writer, value);
}

fn invoke_after_arc_write<F>(lock: &ArcLock, callback: F)
where
    F: Fn(&ArcLock),
{
    set_arc_writer(lock, 0);
    set_arc_writer(lock, 1);
    callback(lock);
}

fn local_arc_does_not_alias_parameter<F>(lock: &ArcLock, callback: F)
where
    F: Fn(&ArcLock),
{
    assume!(get_model_field!(&*lock.lock, writer, 0usize) == 0);
    let local_lock = Arc::new(Lock);
    set_model_field!(&*local_lock, writer, 1usize);
    callback(lock);
}

pub fn arc_write_reaches_callback(lock: &ArcLock) {
    invoke_after_arc_write(lock, acquire_arc_write); //~ possible unsatisfied precondition
}

pub fn local_arc_stays_unrelated(lock: &ArcLock) {
    local_arc_does_not_alias_parameter(lock, acquire_arc_write);
}

pub fn main() {
    let lock = Lock;
    invoke_twice(&lock, acquire_write);
}
