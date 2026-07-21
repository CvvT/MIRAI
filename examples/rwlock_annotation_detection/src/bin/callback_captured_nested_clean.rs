use rwlock_annotation_detection::ModeledRwLock;

fn invoke(callback: impl FnOnce()) {
    callback();
}

fn invoke_nested_callback(callback: impl FnOnce()) {
    invoke(|| callback());
}

fn without_write_held(lock: &ModeledRwLock<()>, callback: impl FnOnce()) {
    let writer = lock.write();
    drop(writer);
    callback();
}

fn main() {
    let lock = ModeledRwLock::new(());
    without_write_held(&lock, || {
        invoke_nested_callback(|| {
            let reader = lock.read();
            std::hint::black_box(reader);
        });
    });
}
