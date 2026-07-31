// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use mirai_annotations::*;

fn requires_distinct(left: &&i32, right: &&i32) {
    precondition!(not_alias!(*left, *right));
    //~ related location
}

pub fn call_with_assumed_referent_alias(left: &i32, right: &i32) {
    let left_holder = left;
    let right_holder = right;
    assumed_alias!(left, right);
    requires_distinct(&left_holder, &right_holder); //~ unsatisfied precondition
}
