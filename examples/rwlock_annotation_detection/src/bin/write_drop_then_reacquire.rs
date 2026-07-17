use rwlock_annotation_detection::ModeledRwLock;

fn main() {
    let lock = ModeledRwLock::new(());

    let writer = lock.write();
    drop(writer);

    let reader = lock.read();
    drop(reader);

    let writer = lock.write();
    std::hint::black_box(writer);
}
