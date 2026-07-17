#![allow(unexpected_cfgs)]

use mirai_annotations::{get_model_field, precondition, set_model_field};
use rwlock_annotation_detection::ModeledRwLock;

fn annotated_acquire(lock: &ModeledRwLock<()>) {
    precondition!(
        get_model_field!(lock, writer, 0usize) == 0,
        "annotated callback requires no live writer"
    );
    set_model_field!(lock, writer, 1usize);
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
    invoke_twice(&lock, annotated_acquire);
}
