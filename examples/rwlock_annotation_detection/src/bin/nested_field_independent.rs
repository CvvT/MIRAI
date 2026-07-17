use rwlock_annotation_detection::ModeledRwLock;

struct Holder {
    lock: ModeledRwLock<()>,
}

fn main() {
    let first = Holder {
        lock: ModeledRwLock::new(()),
    };
    let second = Holder {
        lock: ModeledRwLock::new(()),
    };

    let first_writer = first.lock.acquire_write();
    let second_writer = second.lock.acquire_write();
    std::hint::black_box((first_writer, second_writer));
}
