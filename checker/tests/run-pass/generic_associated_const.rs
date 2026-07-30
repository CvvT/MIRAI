// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

trait HasInit {
    const INIT: Self;
}

trait Provider {
    type Resource: HasInit;
}

fn use_generic_init<P: Provider>() {
    let _ = <P::Resource as HasInit>::INIT;
}

pub fn main() {}
