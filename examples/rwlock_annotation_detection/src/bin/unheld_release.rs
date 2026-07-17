use rwlock_annotation_detection::ModeledRwLock;

fn main() {
    let lock = ModeledRwLock::<()>::new(());
    lock.release_read();
}
