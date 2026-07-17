use rwlock_annotation_detection::ModeledRwLock;
use std::sync::Arc;

fn main() {
    let first = Arc::new(ModeledRwLock::new(()));
    let second = Arc::new(ModeledRwLock::new(()));

    let first_writer = first.acquire_write();
    let second_writer = second.acquire_write();
    std::hint::black_box((first_writer, second_writer));
}
