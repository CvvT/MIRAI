use rwlock_annotation_detection::ModeledRwLock;

fn main() {
    let locks = [ModeledRwLock::new(()), ModeledRwLock::new(())];

    let reader = locks[0].read();
    let writer = locks[0].acquire_write();
    std::hint::black_box((&reader, writer));
}
