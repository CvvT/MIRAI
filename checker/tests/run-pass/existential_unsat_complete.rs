// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use mirai_annotations::precondition;

fn require_comparison_trichotomy(left: i32, right: i32) {
    precondition!((left < right) | (left == right) | (left > right));
}

pub fn call_with_symbolic_arguments(left: i32, right: i32) {
    require_comparison_trichotomy(left, right);
}
