// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// MIRAI_FLAGS --diag=default

use mirai_annotations::*;

struct Lock;

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

    fn invoke(&self, callback: impl FnOnce()) {
        set_model_field!(self.lock(), writer, 1usize);
        Some(()).map(|_| callback()).unwrap();
        set_model_field!(self.lock(), writer, 0usize);
    }

    fn require_unlocked(&self) {
        precondition!(get_model_field!(self.lock(), writer, 0usize) == 0);
    }
}

static FIRST_INNER: LookalikeInner = LookalikeInner {
    strong: 1,
    weak: 1,
    data: Data { lock: Lock },
};

static SECOND_INNER: LookalikeInner = LookalikeInner {
    strong: 1,
    weak: 1,
    data: Data { lock: Lock },
};

pub fn non_arc_layout_does_not_activate_carrier() {
    let first = Owner(sync::Arc(LookalikePointer(&FIRST_INNER)));
    let second = Owner(sync::Arc(LookalikePointer(&SECOND_INNER)));
    first.invoke(|| second.require_unlocked());
}

pub fn main() {}
