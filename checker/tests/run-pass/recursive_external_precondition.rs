// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// MIRAI_FLAGS --body_analysis_timeout 60 --crate_analysis_timeout 120

use mirai_annotations::*;

fn recursive_require_nonzero(value: i32, recurse: bool) {
    arc_reexport::require_nonzero(value);
    if recurse {
        recursive_require_nonzero(value, false);
    }
    // Together with require_nonzero, these fill the 50-precondition cap exactly. The final
    // diagnostic disappears if recursive promotion consumes even one duplicate slot.
    precondition!(value >= 0, "bound 0");
    precondition!(value >= 1, "bound 1");
    precondition!(value >= 2, "bound 2");
    precondition!(value >= 3, "bound 3");
    precondition!(value >= 4, "bound 4");
    precondition!(value >= 5, "bound 5");
    precondition!(value >= 6, "bound 6");
    precondition!(value >= 7, "bound 7");
    precondition!(value >= 8, "bound 8");
    precondition!(value >= 9, "bound 9");
    precondition!(value >= 10, "bound 10");
    precondition!(value >= 11, "bound 11");
    precondition!(value >= 12, "bound 12");
    precondition!(value >= 13, "bound 13");
    precondition!(value >= 14, "bound 14");
    precondition!(value >= 15, "bound 15");
    precondition!(value >= 16, "bound 16");
    precondition!(value >= 17, "bound 17");
    precondition!(value >= 18, "bound 18");
    precondition!(value >= 19, "bound 19");
    precondition!(value >= 20, "bound 20");
    precondition!(value >= 21, "bound 21");
    precondition!(value >= 22, "bound 22");
    precondition!(value >= 23, "bound 23");
    precondition!(value >= 24, "bound 24");
    precondition!(value >= 25, "bound 25");
    precondition!(value >= 26, "bound 26");
    precondition!(value >= 27, "bound 27");
    precondition!(value >= 28, "bound 28");
    precondition!(value >= 29, "bound 29");
    precondition!(value >= 30, "bound 30");
    precondition!(value >= 31, "bound 31");
    precondition!(value >= 32, "bound 32");
    precondition!(value >= 33, "bound 33");
    precondition!(value >= 34, "bound 34");
    precondition!(value >= 35, "bound 35");
    precondition!(value >= 36, "bound 36");
    precondition!(value >= 37, "bound 37");
    precondition!(value >= 38, "bound 38");
    precondition!(value >= 39, "bound 39");
    precondition!(value >= 40, "bound 40");
    precondition!(value >= 41, "bound 41");
    precondition!(value >= 42, "bound 42");
    precondition!(value >= 43, "bound 43");
    precondition!(value >= 44, "bound 44");
    precondition!(value >= 45, "bound 45");
    precondition!(value >= 46, "bound 46");
    precondition!(value >= 47, "bound 47");
    precondition!(value >= 48, "bound 48"); //~ related location
}

pub fn trigger() {
    recursive_require_nonzero(47, true); //~ bound 48
}

pub fn main() {}
