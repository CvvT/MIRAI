use rwlock_annotation_detection::ModeledRwLock;

fn without_write_held<F>(lock: &ModeledRwLock<()>, callback: F)
where
    F: FnOnce(&ModeledRwLock<()>),
{
    let writer = lock.write();
    drop(writer);
    callback(lock);
}

fn through_adapter<F>(lock: &ModeledRwLock<()>, callback: F)
where
    F: FnOnce(&ModeledRwLock<()>),
{
    without_write_held(lock, |lock| callback(lock));
}

fn main() {
    let lock = ModeledRwLock::new(());
    through_adapter(&lock, |lock| {
        let reader = lock.read();
        std::hint::black_box(reader);
    });
}
