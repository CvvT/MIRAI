use rwlock_annotation_detection::ModeledRwLock;

fn invoke<F>(callback: F)
where
    F: FnOnce(),
{
    callback();
}

fn main() {
    let lock = ModeledRwLock::new(());
    let held = lock.acquire_write();

    invoke(|| {
        let reentrant = lock.acquire_write();
        std::hint::black_box(reentrant);
    });

    std::hint::black_box(held);
}
