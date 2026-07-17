use rwlock_annotation_detection::ModeledRwLock;

struct Holder {
    lock: ModeledRwLock<()>,
}

fn main() {
    let holder = Holder {
        lock: ModeledRwLock::new(()),
    };

    let first = holder.lock.acquire_write();
    let second = holder.lock.acquire_write();
    std::hint::black_box((first, second));
}
