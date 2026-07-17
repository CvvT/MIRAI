use rwlock_annotation_detection::ModeledRwLock;

fn clean(lock: &ModeledRwLock<()>) {
    std::hint::black_box(lock);
}

fn read(lock: &ModeledRwLock<()>) {
    let reader = lock.read();
    std::hint::black_box(reader);
}

fn with_write_held(lock: &ModeledRwLock<()>, callback: fn(&ModeledRwLock<()>)) {
    let writer = lock.write();
    callback(lock);
    drop(writer);
}

fn main() {
    let lock = ModeledRwLock::new(());
    with_write_held(&lock, clean);
    with_write_held(&lock, read);
}
