// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// MIRAI_FLAGS --diag=default
//
// The unspecialized callback-discovery pass must not leak its diagnostics into the specialized
// analysis. Carrier replay is defensively scoped as well, but independent roots cannot consume
// one another's carried state, so that isolation has no constructible behavioral witness here.

use mirai_annotations::*;
use std::sync::Arc;

pub struct Lock;
pub struct Options;

struct Inner {
    lock: Lock,
}

pub struct Owner {
    inner: Arc<Inner>,
}

pub struct PlainOwner {
    lock: Lock,
}

impl Owner {
    fn lock(&self) -> &Lock {
        &self.inner.lock
    }

    fn options(&self) -> OptionsGuard {
        OptionsGuard { options: Options }
    }

    fn with_locked_options<R>(&self, callback: impl FnOnce(&mut Options) -> R) -> R {
        set_model_field!(self.lock(), writer, 1usize);
        let result = self.options().with_options(|options| callback(options));
        set_model_field!(self.lock(), writer, 0usize);
        result
    }
}

impl PlainOwner {
    fn with_options<R>(&self, callback: impl FnOnce(&mut Options) -> R) -> R {
        OptionsGuard { options: Options }.with_options(|options| callback(options))
    }

    fn require_unlocked(&self) {
        precondition!(get_model_field!(&self.lock, writer, 0usize) == 0);
    }
}

struct OptionsGuard {
    options: Options,
}

impl OptionsGuard {
    fn with_options<R>(&mut self, callback: impl FnOnce(&mut Options) -> R) -> R {
        map_options(&mut self.options, |options| callback(options))
    }
}

fn map_options<R>(options: &mut Options, callback: impl FnOnce(&mut Options) -> R) -> R {
    Some(options).map(|options| callback(options)).unwrap()
}

pub fn repeated_static_site_is_scoped_per_call(first: &Owner, second: &PlainOwner) {
    first.with_locked_options(|_options| {});
    second.with_options(|_options| second.require_unlocked());
}

pub fn main() {}
