// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// MIRAI_FLAGS --diag=default

use std::ptr::NonNull;

pub fn cast_to_raw_pointer(pointer: NonNull<u64>) -> *mut u64 {
    unsafe { std::mem::transmute(pointer) }
}

pub fn main() {}
