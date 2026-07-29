// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// MIRAI_FLAGS --diag=default

mod reexport {
    pub use std::sync::Arc;
}

pub struct Data {
    value: u64,
}

pub fn read_through_reexport(data: &reexport::Arc<Data>) -> u64 {
    data.value
}

pub fn main() {}
