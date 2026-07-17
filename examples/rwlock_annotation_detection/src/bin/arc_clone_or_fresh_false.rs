use rwlock_annotation_detection::ModeledRwLock;
use std::sync::Arc;

fn clone_or_fresh(lock: &Arc<ModeledRwLock<()>>, clone: bool) -> Arc<ModeledRwLock<()>> {
    if clone {
        Arc::clone(lock)
    } else {
        Arc::new(ModeledRwLock::new(()))
    }
}

fn main() {
    let first = Arc::new(ModeledRwLock::new(()));
    let second = clone_or_fresh(&first, false);

    let first_writer = first.acquire_write();
    let second_writer = second.acquire_write();
    std::hint::black_box((first_writer, second_writer));
}
