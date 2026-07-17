use rwlock_annotation_detection::ModeledRwLock;

struct Pair {
    a: ModeledRwLock<()>,
    b: ModeledRwLock<()>,
}

fn main() {
    let pair = Pair {
        a: ModeledRwLock::new(()),
        b: ModeledRwLock::new(()),
    };

    let first = pair.a.acquire_write();
    let second = pair.a.acquire_write();
    std::hint::black_box((first, second, &pair.b));
}
