#![allow(unexpected_cfgs)]

use mirai_annotations::{get_model_field, precondition, set_model_field};
use rwlock_annotation_detection::ModeledRwLock;

fn require_zero_then_increment(lock: &ModeledRwLock<()>) {
    let count = get_model_field!(lock, callback_count, 0usize);
    precondition!(count == 0, "counter callback requires zero");
    set_model_field!(lock, callback_count, count + 1);
}

fn invoke_twice<F>(lock: &ModeledRwLock<()>, callback: F)
where
    F: Fn(&ModeledRwLock<()>),
{
    callback(lock);
    callback(lock);
}

fn main() {
    let lock = ModeledRwLock::new(());
    invoke_twice(&lock, require_zero_then_increment);
}
