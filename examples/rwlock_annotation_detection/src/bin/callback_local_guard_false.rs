#![allow(unexpected_cfgs)]

use mirai_annotations::{get_model_field, precondition};
use rwlock_annotation_detection::ModeledRwLock;

fn require_unlocked(lock: &ModeledRwLock<()>) {
    precondition!(
        get_model_field!(lock, write_held, 0usize) == 0,
        "local-guard callback requires no live writer"
    );
}

fn invoke_if_local<F>(lock: &ModeledRwLock<()>, callback: F)
where
    F: FnOnce(&ModeledRwLock<()>),
{
    let writer = lock.write();
    let invoke = std::hint::black_box(false);
    if invoke {
        callback(lock);
    }
    drop(writer);
}

fn main() {
    let lock = ModeledRwLock::new(());
    invoke_if_local(&lock, require_unlocked);
}
