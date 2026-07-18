#![allow(unexpected_cfgs)]

use mirai_annotations::{get_model_field, precondition, set_model_field};
use rwlock_annotation_detection::ModeledRwLock;

fn increment_read_count(lock: &ModeledRwLock<()>) {
    let read_count = get_model_field!(lock, read_count, 0usize);
    set_model_field!(lock, read_count, read_count + 1);
}

fn require_one_reader(lock: &ModeledRwLock<()>) {
    precondition!(
        get_model_field!(lock, read_count, 0usize) == 1,
        "second callback requires one reader"
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
    invoke_in_order(&lock, increment_read_count, require_one_reader);
}
