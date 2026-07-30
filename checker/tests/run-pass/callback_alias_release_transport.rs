// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// An alias established inside an outer callback must expose an inner callback's final model-field
// state to subsequent operations. The incidental enum argument keeps this close to callbacks that
// parse an option before mutating aliased state.

use mirai_annotations::*;
use std::sync::Arc;

struct Lock;

pub struct State {
    write_handle: Arc<Lock>,
    read_handle: Arc<Lock>,
}

pub enum Incidental {
    Value(u32),
}

fn acquire(lock: &Lock) {
    set_model_field!(lock, writer, 1usize);
}

fn release(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
}

fn read(lock: &Lock) {
    precondition!(get_model_field!(lock, writer, 0usize) == 0);
}

fn with_write<F: FnOnce()>(state: &State, callback: F) {
    acquire(&state.write_handle);
    callback();
    release(&state.write_handle);
}

fn invoke<F: FnOnce(Incidental)>(incidental: Incidental, callback: F) {
    callback(incidental);
}

pub fn trigger_fixed(state: &State, incidental: Incidental) {
    invoke(incidental, |_incidental| {
        assumed_alias!(&*state.read_handle, &*state.write_handle);
        with_write(state, || {});
        read(&state.read_handle);
    });
}

pub fn trigger_buggy(state: &State, incidental: Incidental) {
    invoke(incidental, |_incidental| {
        assumed_alias!(&*state.read_handle, &*state.write_handle);
        acquire(&state.write_handle);
        read(&state.read_handle);
    }); //~ unsatisfied precondition
}

pub fn trigger_read_before_release(state: &State, incidental: Incidental) {
    invoke(incidental, |_incidental| {
        assumed_alias!(&*state.read_handle, &*state.write_handle);
        with_write(state, || {
            read(&state.read_handle); //~ unsatisfied precondition
        });
    });
}

pub fn main() {}
