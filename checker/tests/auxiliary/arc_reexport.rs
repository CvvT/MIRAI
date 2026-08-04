// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

pub use std::sync::Arc;

pub fn require_nonzero(value: i32) {
    assert!(value != 0);
}
