// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// MIRAI_FLAGS --diag=default

use mirai_annotations::*;

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

mod sync {
    use super::LookalikePointer;

    pub struct Arc(pub LookalikePointer);
}

struct Owner(sync::Arc);

impl Owner {
    fn lock(&self) -> &Lock {
        &self.0.0.0.data.lock
    }

    fn options(&self) -> OptionsGuard<'_> {
        set_model_field!(self.lock(), writer, 1usize);
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

impl Drop for OptionsGuard<'_> {
    fn drop(&mut self) {
        set_model_field!(self.owner.lock(), writer, 0usize);
    }
}

fn map_options<R>(options: &mut Options, callback: impl FnOnce(&mut Options) -> R) -> R {
    Some(options).map(|options| callback(options)).unwrap()
}

static INNER: LookalikeInner = LookalikeInner {
    strong: 1,
    weak: 1,
    data: Data { lock: Lock },
};

pub fn non_arc_layout_does_not_activate_carrier() {
    let owner = Owner(sync::Arc(LookalikePointer(&INNER)));
    owner.with_options(|_options| owner.require_unlocked());
}

pub fn main() {}
