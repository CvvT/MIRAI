use rwlock_annotation_detection::ModeledRwLock;

struct Outer {
    mid: Mid,
}

struct Mid {
    lock: ModeledRwLock<()>,
}

fn main() {
    let first = Outer {
        mid: Mid {
            lock: ModeledRwLock::new(()),
        },
    };
    let second = Outer {
        mid: Mid {
            lock: ModeledRwLock::new(()),
        },
    };

    let first_writer = first.mid.lock.acquire_write();
    let second_writer = second.mid.lock.acquire_write();
    std::hint::black_box((first_writer, second_writer));
}
