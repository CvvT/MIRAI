use rwlock_annotation_detection::ModeledRwLock;

fn invoke<F>(callback: F)
where
    F: FnOnce(),
{
    callback();
}

fn main() {
    let lock = ModeledRwLock::new(());
    invoke(|| {
        let reader = lock.read();
        drop(reader);
    });
}
