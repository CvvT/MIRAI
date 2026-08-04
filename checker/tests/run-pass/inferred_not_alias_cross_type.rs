// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the root directory of this source tree.

// A Network and a Lock cannot alias as Rust locations. Without the terminal-type gate, alias
// inference nevertheless adds not_alias(network, lock) to cross_type_candidate. The transmute
// makes that vacuous internal predicate false at the caller and exposes it as a spurious warning.

use mirai_annotations::*;

pub struct Network;
pub struct Lock;

fn require_unlocked(lock: &Lock) {
    precondition!(get_model_field!(lock, writer, 0usize) == 0);
}

fn cross_type_candidate(network: &Network, lock: &Lock) {
    set_model_field!(network, writer, 1usize);
    require_unlocked(lock);
}

pub fn transmuted_reference_does_not_create_a_cross_type_obligation(network: &Network) {
    set_model_field!(network, writer, 0usize);
    let lock: &Lock = unsafe { std::mem::transmute(network) };
    cross_type_candidate(network, lock);
}

pub fn main() {}
