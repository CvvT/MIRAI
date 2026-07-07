#![allow(unexpected_cfgs)]

//! Annotation-style rlock example: acquiring the same receiver-relative database lock twice
//! violates the reviewed acquire contract.
//!
//! NOTE: This is developer-reviewable lock-discipline *spec*, encoded with the existing
//! `mirai-annotations`/`contracts` surface. A lock-native MIRAI diagnostic (a lock-typed
//! DoubleLock/OrderViolation/UnheldRelease) is the known, separately-scoped residual; today
//! MIRAI emits a generic precondition/verify violation, not a lock-typed one.

use contracts::*;
use mirai_annotations::*;

#[requires(!database_held, "database lock must not already be held")]
#[ensures(ret, "database lock is held after acquire")]
fn acquire_database(database_held: bool) -> bool {
    let _ = database_held;
    true
}

fn double_acquire() {
    let database_held = acquire_database(false);
    let _database_held = acquire_database(database_held);
}

fn main() {
    if cfg!(mirai) {
        double_acquire();
    }
    println!(
        "double_acquire: spec encodes the second acquire as a precondition violation \
         (generic, not lock-typed)"
    );
}
