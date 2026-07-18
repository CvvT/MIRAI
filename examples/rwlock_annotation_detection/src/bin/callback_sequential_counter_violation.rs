#![allow(unexpected_cfgs)]

use mirai_annotations::{get_model_field, precondition, set_model_field};
use rwlock_annotation_detection::ModeledRwLock;

fn increment_read_count(lock: &ModeledRwLock<()>) {
    let read_count = get_model_field!(lock, read_count, 0usize);
    set_model_field!(lock, read_count, read_count + 1);
}

fn incorrectly_require_two_readers(lock: &ModeledRwLock<()>) {
    precondition!(
        get_model_field!(lock, read_count, 0usize) == 2,
        "second callback incorrectly requires two readers"
    );
}

fn invoke_in_order<F, G>(lock: &ModeledRwLock<()>, first: F, second: G)
where
    F: Fn(&ModeledRwLock<()>),
    G: Fn(&ModeledRwLock<()>),
{
    first(lock);
    second(lock);
}

fn main() {
    let lock = ModeledRwLock::new(());
    invoke_in_order(
        &lock,
        increment_read_count,
        incorrectly_require_two_readers,
    );
}
