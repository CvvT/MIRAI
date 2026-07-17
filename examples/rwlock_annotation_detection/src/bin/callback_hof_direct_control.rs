#![allow(unexpected_cfgs)]

use mirai_annotations::{get_model_field, precondition, set_model_field};
use rwlock_annotation_detection::ModeledRwLock;

fn annotated_acquire(lock: &ModeledRwLock<()>) {
    precondition!(
        get_model_field!(lock, write_held, 0usize) == 0,
        "annotated callback requires no live writer"
    );
    set_model_field!(lock, write_held, 1usize);
}

fn main() {
    let lock = ModeledRwLock::new(());
    annotated_acquire(&lock);
    annotated_acquire(&lock);
}
