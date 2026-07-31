// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the root directory of this source tree.

use mirai_annotations::*;

fn requires_distinct(left: &i32, right: &i32) {
    precondition!(not_alias!(left, right));
    //~ related location
}

pub fn call_with_same_value(value: &i32) {
    requires_distinct(value, value); //~ unsatisfied precondition
}
