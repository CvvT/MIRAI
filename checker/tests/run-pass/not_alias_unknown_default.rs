// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the root directory of this source tree.

// MIRAI_FLAGS --diag=default

use mirai_annotations::*;

fn requires_distinct(left: &i32, right: &i32) {
    precondition!(not_alias!(left, right));
}

pub fn call_with_unconstrained_values(left: &i32, right: &i32) {
    requires_distinct(left, right);
}
