use rwlock_annotation_detection::ModeledRwLock;

fn main() {
    let lock = ModeledRwLock::new(());
    let writer = lock.write();
    let reader = lock.read();
    std::hint::black_box((&writer, &reader));
}
