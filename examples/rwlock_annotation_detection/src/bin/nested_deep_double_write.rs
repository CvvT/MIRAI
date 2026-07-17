use rwlock_annotation_detection::ModeledRwLock;

struct Outer {
    mid: Mid,
}

struct Mid {
    lock: ModeledRwLock<()>,
}

fn main() {
    let outer = Outer {
        mid: Mid {
            lock: ModeledRwLock::new(()),
        },
    };

    let first = outer.mid.lock.acquire_write();
    let second = outer.mid.lock.acquire_write();
    std::hint::black_box((first, second));
}
