// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the root directory of this source tree.

// MIRAI_FLAGS --diag=default

use mirai_annotations::*;

pub struct Lock;

pub struct Network<'a> {
    lock: &'a Lock,
}

pub struct Task<'a> {
    net: Network<'a>,
    litebox: &'a Lock,
}

fn read_network_while_descriptor_writer_is_held(descriptor_lock: &Lock, network_lock: &Lock) {
    precondition!(not_alias!(descriptor_lock, network_lock)); //~ related location
}

fn setsockopt_buggy(task: &Task<'_>) {
    read_network_while_descriptor_writer_is_held(task.litebox, task.net.lock); //~ related location
}

fn dispatch_setsockopt_buggy(task: &Task<'_>) {
    setsockopt_buggy(task); //~ related location
}

fn setsockopt_fixed(_task: &Task<'_>) {
    // The descriptor writer has been released before the network access, so there is no
    // live-writer non-alias obligation to propagate.
}

fn dispatch_setsockopt_fixed(task: &Task<'_>) {
    setsockopt_fixed(task);
}

pub fn do_syscall_buggy(task: &Task<'_>) {
    assumed_alias!(task.net.lock, task.litebox);
    dispatch_setsockopt_buggy(task); //~ unsatisfied precondition
}

pub fn do_syscall_fixed(task: &Task<'_>) {
    assumed_alias!(task.net.lock, task.litebox);
    dispatch_setsockopt_fixed(task);
}

pub fn main() {}
