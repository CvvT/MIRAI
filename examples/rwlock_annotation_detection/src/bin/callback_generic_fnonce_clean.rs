use rwlock_annotation_detection::ModeledRwLock;

fn invoke_generic<F>(lock: &ModeledRwLock<()>, callback: F)
where
    F: FnOnce(&ModeledRwLock<()>),
{
    let writer = lock.write();
    callback(lock);
    drop(writer);
}

fn main() {
    let lock = ModeledRwLock::new(());
    invoke_generic(&lock, |lock| {
        std::hint::black_box(lock);
    });
}
