#![allow(unexpected_cfgs)]

//! Annotation-style rlock example: all receiver-local acquisitions follow explicitly reviewed
//! database < cache < disk and database < disk order edges.
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

#[requires(!cache_held, "cache lock must not already be held")]
#[requires(
    !database_held || database_before_cache,
    "reviewed order edge database < cache is required"
)]
#[ensures(ret, "cache lock is held after acquire")]
fn acquire_cache(cache_held: bool, database_held: bool, database_before_cache: bool) -> bool {
    let _ = (cache_held, database_held, database_before_cache);
    true
}

#[requires(!disk_held, "disk lock must not already be held")]
#[requires(
    !database_held || database_before_disk,
    "reviewed order edge database < disk is required"
)]
#[requires(
    !cache_held || cache_before_disk,
    "reviewed order edge cache < disk is required"
)]
#[ensures(ret, "disk lock is held after acquire")]
fn acquire_disk(
    disk_held: bool,
    database_held: bool,
    cache_held: bool,
    database_before_disk: bool,
    cache_before_disk: bool,
) -> bool {
    let _ = (
        disk_held,
        database_held,
        cache_held,
        database_before_disk,
        cache_before_disk,
    );
    true
}

fn clean_ordered() {
    let database_held = acquire_database(false);
    let cache_held = acquire_cache(false, database_held, true);
    let disk_held = acquire_disk(false, database_held, cache_held, true, true);

    verify!(database_held);
    verify!(cache_held);
    verify!(disk_held);
}

fn main() {
    clean_ordered();
    println!(
        "clean_ordered: spec satisfies every reviewed order edge \
         (generic preconditions, not lock-typed)"
    );
}
