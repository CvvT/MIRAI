use rwlock_annotation_detection::ModeledRwLock;
use std::sync::Arc;

fn clone_lock(lock: &Arc<ModeledRwLock<()>>) -> Arc<ModeledRwLock<()>> {
    Arc::clone(lock)
}

fn main() {
    let first = Arc::new(ModeledRwLock::new(()));
    let alias = clone_lock(&first);

    let first_writer = first.acquire_write();
    let second_writer = alias.acquire_write();
    std::hint::black_box((first_writer, second_writer));
}
