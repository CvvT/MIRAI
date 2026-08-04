// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// MIRAI_FLAGS --diag=default
//
// The nested name deliberately ends in `::sync::Arc`, but this user-defined type must not activate
// Arc-specific callback model-state carrying because it is not Rust's `Arc` diagnostic item.
// Ordinary callback replay still detects the genuine writer precondition violation below.

use mirai_annotations::*;
use std::ops::{Deref, DerefMut};

pub struct Lock;
pub struct Options;

struct Data {
    lock: Lock,
}

struct LookalikeInner {
    strong: usize,
    weak: usize,
    data: Data,
}

struct LookalikePointer(&'static LookalikeInner);

mod lookalike {
    pub mod sync {
        use super::super::LookalikePointer;

        pub struct Arc(pub LookalikePointer);
    }
}

struct Owner(lookalike::sync::Arc);

impl Owner {
    fn lock(&self) -> &Lock {
        &self.0.0.0.data.lock
    }

    fn acquire_write(&self) {
        set_model_field!(self.lock(), writer, 1usize);
    }

    fn options(&self) -> OptionsGuard<'_> {
        self.acquire_write();
        OptionsGuard {
            owner: self,
            options: Options,
        }
    }

    fn with_options<R>(&self, callback: impl FnOnce(&mut Options) -> R) -> R {
        self.options()
            .with_options(|options| callback(options))
    }

    fn require_unlocked(&self) {
        precondition!(get_model_field!(self.lock(), writer, 0usize) == 0);
    }

    pub fn trigger(&self) {
        self.with_options(|_options| self.require_unlocked()); //~ unsatisfied precondition
    }
}

struct OptionsGuard<'a> {
    owner: &'a Owner,
    options: Options,
}

impl OptionsGuard<'_> {
    fn with_options<R>(&mut self, callback: impl FnOnce(&mut Options) -> R) -> R {
        map_options(&mut self.options, |options| callback(options))
    }
}

impl Deref for OptionsGuard<'_> {
    type Target = Options;

    fn deref(&self) -> &Options {
        &self.options
    }
}

impl DerefMut for OptionsGuard<'_> {
    fn deref_mut(&mut self) -> &mut Options {
        &mut self.options
    }
}

impl Drop for OptionsGuard<'_> {
    fn drop(&mut self) {
        set_model_field!(self.owner.lock(), writer, 0usize);
    }
}

fn map_options<R>(options: &mut Options, callback: impl FnOnce(&mut Options) -> R) -> R {
    Some(options).map(|options| callback(options)).unwrap()
}

pub fn main() {}
