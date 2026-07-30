// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use mirai_annotations::*;

#[allow(dead_code)]
enum Value {
    U32(u32),
    Timeout(u32),
    Text(String),
}

fn require_nonzero(value: u32) {
    precondition!(value != 0);
}

fn dispatch<F: FnOnce(Value)>(callback: F) {
    let value = Value::U32(0);
    callback(value);
}

fn dispatch_unavailable_payload<F: FnOnce(Value)>(callback: F) {
    let value = Value::Text(String::from("local"));
    callback(value);
}

pub fn trigger() {
    dispatch(|value| match value {
        Value::U32(value) => require_nonzero(value),
        Value::Timeout(value) => {
            let _ = value;
        }
        Value::Text(_) => {}
    }); //~ unsatisfied precondition

    dispatch_unavailable_payload(|value| match value {
        Value::Text(_) => require_nonzero(0),
        Value::U32(_) | Value::Timeout(_) => {}
    }); //~ unsatisfied precondition
}

pub fn main() {}
