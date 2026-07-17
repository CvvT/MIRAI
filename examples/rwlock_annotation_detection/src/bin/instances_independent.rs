use rwlock_annotation_detection::ModeledRwLock;

fn main() {
    let a = ModeledRwLock::new(());
    let b = ModeledRwLock::new(());

    let a_reader = a.read();
    let b_writer = b.acquire_write();
    std::hint::black_box((&a_reader, b_writer));
}
