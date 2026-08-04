// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the root directory of this source tree.

// Exercises live call-site NotAlias inference and proves it is load-bearing.
//
// `require_zero` has a precondition on `a.writer`. Inside `distinct_writers` a
// live, conflicting writer value is held on `b` (`b.writer == 1`) while
// `a.writer` stays symbolic, so `require_zero(a)` is only unsatisfiable when `a`
// and `b` alias. The hook infers `NotAlias(a, b)` as a precondition of
// `distinct_writers` (alongside the ordinary promoted `a.writer == 0`).
//
// `same_lock` first discharges the ordinary precondition by setting
// `lock.writer == 0`, then calls `distinct_writers(lock, lock)` with aliased
// arguments. The promoted `a.writer == 0` is satisfied, so the only obligation
// that can fail is the inferred `NotAlias(lock, lock)`, which is definitely
// false. With the inference hook disabled `distinct_writers` only carries the
// (now-discharged) `a.writer == 0` precondition and this call reports nothing,
// so the fixture only passes because the hook is active.

use mirai_annotations::*;

pub struct Lock;

fn require_zero(lock: &Lock) {
    precondition!(get_model_field!(lock, writer, 0usize) == 0); //~ related location
}

fn distinct_writers(a: &Lock, b: &Lock) {
    set_model_field!(b, writer, 1usize);
    require_zero(a); //~ related location
}

pub fn same_lock(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    distinct_writers(lock, lock); //~ possible alias violates precondition
}

pub fn main() {}
