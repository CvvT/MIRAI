use rwlock_annotation_detection::ModeledRwLock;

fn main() {
    let locks = [ModeledRwLock::new(()), ModeledRwLock::new(())];

    let first_reader = locks[0].read();
    let second_writer = locks[1].acquire_write();
    std::hint::black_box((&first_reader, second_writer));
}
