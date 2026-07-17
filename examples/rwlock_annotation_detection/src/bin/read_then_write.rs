use rwlock_annotation_detection::ModeledRwLock;

fn main() {
    let lock = ModeledRwLock::new(());
    let reader = lock.read();
    let writer = lock.acquire_write();
    std::hint::black_box((&reader, writer));
}
