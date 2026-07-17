use rwlock_annotation_detection::ModeledRwLock;

fn main() {
    let lock = ModeledRwLock::new(());
    let first = lock.acquire_write();
    let second = first.acquire_write();
    std::hint::black_box(second);
}
