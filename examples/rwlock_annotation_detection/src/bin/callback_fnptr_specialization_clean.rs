use rwlock_annotation_detection::ModeledRwLock;

fn first(lock: &ModeledRwLock<()>) {
    std::hint::black_box(lock);
}

fn second(lock: &ModeledRwLock<()>) {
    std::hint::black_box((lock, 1usize));
}

fn with_write_held(lock: &ModeledRwLock<()>, callback: fn(&ModeledRwLock<()>)) {
    let writer = lock.write();
    callback(lock);
    drop(writer);
}

fn main() {
    let lock = ModeledRwLock::new(());
    with_write_held(&lock, first);
    with_write_held(&lock, second);
}
