#![allow(unexpected_cfgs)]

//! Annotation-style rlock example: releasing a receiver-relative database lock without holding it
//! violates the reviewed release contract.
//!
//! NOTE: This is developer-reviewable lock-discipline *spec*, encoded with the existing
//! `mirai-annotations`/`contracts` surface. A lock-native MIRAI diagnostic (a lock-typed
//! DoubleLock/OrderViolation/UnheldRelease) is the known, separately-scoped residual; today
//! MIRAI emits a generic precondition/verify violation, not a lock-typed one.

use contracts::*;
use mirai_annotations::*;

#[requires(database_held, "database lock must be held to release")]
#[ensures(!ret, "database lock is released")]
fn release_database(database_held: bool) -> bool {
    let _ = database_held;
    false
}

fn unheld_release() {
    let _database_held = release_database(false);
}

fn main() {
    if cfg!(mirai) {
        unheld_release();
    }
    println!(
        "unheld_release: spec encodes the unheld release as a precondition violation \
         (generic, not lock-typed)"
    );
}
