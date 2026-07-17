use rwlock_annotation_detection::ModeledRwLock;

fn with_write_held<F>(lock: &ModeledRwLock<()>, callback: F)
where
    F: FnOnce(&ModeledRwLock<()>),
{
    let writer = lock.write();
    callback(lock);
    drop(writer);
}

fn main() {
    let lock = ModeledRwLock::new(());
    with_write_held(&lock, |lock| {
        std::hint::black_box(lock);
    });
    with_write_held(&lock, |lock| {
        let reader = lock.read();
        std::hint::black_box(reader);
    });
}
