use rwlock_annotation_detection::ModeledRwLock;

fn main() {
    let lock = ModeledRwLock::new(());
    let first = lock.read();
    let second = lock.read();
    drop(first);

    let writer = lock.write();
    std::hint::black_box((&second, &writer));
}
