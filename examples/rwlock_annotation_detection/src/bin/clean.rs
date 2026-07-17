use rwlock_annotation_detection::ModeledRwLock;

fn main() {
    let lock = ModeledRwLock::new(());

    let first = lock.read();
    let second = lock.read();
    std::hint::black_box((&first, &second));
    drop(first);
    drop(second);
}
