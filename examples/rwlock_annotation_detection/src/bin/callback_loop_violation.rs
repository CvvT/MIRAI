#![allow(unexpected_cfgs)]

use mirai_annotations::{get_model_field, precondition, set_model_field};
use rwlock_annotation_detection::ModeledRwLock;

fn acquire_once(lock: &ModeledRwLock<()>) {
    precondition!(
        get_model_field!(lock, write_held, 0usize) == 0,
        "loop callback requires no live writer"
    );
    set_model_field!(lock, write_held, 1usize);
}

fn invoke_in_loop<F>(lock: &ModeledRwLock<()>, callback: F)
where
    F: Fn(&ModeledRwLock<()>),
{
    for _ in 0..2 {
        callback(lock);
    }
}

fn main() {
    let first = ModeledRwLock::new(());
    invoke_in_loop(&first, acquire_once);
    let second = ModeledRwLock::new(());
    invoke_in_loop(&second, acquire_once);
}
