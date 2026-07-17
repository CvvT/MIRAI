use rwlock_annotation_detection::ModeledRwLock;

fn acquire_write(lock: &ModeledRwLock<()>) {
    let writer = lock.acquire_write();
    std::hint::black_box(writer);
}

fn invoke(callback: fn(&ModeledRwLock<()>), lock: &ModeledRwLock<()>) {
    callback(lock);
}

fn main() {
    let lock = ModeledRwLock::new(());
    invoke(acquire_write, &lock);
    invoke(acquire_write, &lock);
}
