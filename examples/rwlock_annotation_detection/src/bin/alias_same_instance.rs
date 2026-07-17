use rwlock_annotation_detection::ModeledRwLock;

fn main() {
    let a = ModeledRwLock::new(());
    let alias = &a;

    let reader = a.read();
    let writer = alias.acquire_write();
    std::hint::black_box((&reader, writer));
}
