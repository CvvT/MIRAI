use rwlock_annotation_detection::ModeledRwLock;

fn invoke<F>(lock: &ModeledRwLock<()>, callback: F)
where
    F: FnOnce(&ModeledRwLock<()>),
{
    let writer = lock.write();
    Some(lock).map(callback);
    drop(writer);
}

fn invoke_rewrapped<F>(lock: &ModeledRwLock<()>, callback: F)
where
    F: FnOnce(&ModeledRwLock<()>),
{
    invoke(lock, |value| callback(value));
}

fn main() {
    let lock = ModeledRwLock::new(());
    invoke_rewrapped(&lock, |lock| {
        let reader = lock.read();
        std::hint::black_box(reader);
    });
}
