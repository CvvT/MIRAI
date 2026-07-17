#![allow(unexpected_cfgs)]

use mirai_annotations::{get_model_field, precondition};
use rwlock_annotation_detection::ModeledRwLock;

fn require_unlocked(lock: &ModeledRwLock<()>) {
    precondition!(
        get_model_field!(lock, write_held, 0usize) == 0,
        "conditional callback requires no live writer"
    );
}

fn invoke_if<F>(enabled: bool, lock: &ModeledRwLock<()>, callback: F)
where
    F: FnOnce(&ModeledRwLock<()>),
{
    let writer = lock.write();
    if enabled {
        callback(lock);
    }
    drop(writer);
}

fn main() {
    let lock = ModeledRwLock::new(());
    invoke_if(true, &lock, require_unlocked);
}
