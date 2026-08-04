// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.
//

// A test that uses a function parameter

use mirai_annotations::*;

fn bar(x: Option<i32>, j: i32) -> Option<i32> {
    match x {
        Some(i) => Some(i << 1),
        None => Some(j),
    }
}

fn bas<T>(x: Option<T>, j: T) -> Option<T> {
    match x {
        Some(i) => Some(i),
        None => Some(j),
    }
}

fn foo(f: fn(Option<i32>, i32) -> Option<i32>) -> Option<i32> {
    f(Some(1), 1)
}

fn foos<T: Copy>(f: fn(Option<T>, T) -> Option<T>, x: T) -> Option<T> {
    f(Some(x), x)
}

struct S {
    pub i: i32,
    pub f: fn(Option<i32>, i32) -> Option<i32>,
}

fn foot(s: S) -> Option<i32> {
    (s.f)(Some(s.i), s.i)
}

pub fn main() {
    let fbar = foo(bar);
    // Known limitation: calls through a function parameter are replayed as callback events without
    // a return value. Direct and generic parameter results remain unknown; the structured field
    // function pointer below resolves normally.
    verify!(fbar.unwrap() == 2); //~ possible called `Option::unwrap()` on a `None` value
    //~ possible false verification condition
    //~ related location
    let fbas = foos(bas, 2);
    verify!(fbas.unwrap() == 2); //~ possible false verification condition
    let fbart = foot(S { i: 2, f: bar });
    verify!(fbart.unwrap() == 4);
}
