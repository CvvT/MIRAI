use rwlock_annotation_detection::ModeledRwLock;

fn main() {
    let lock = ModeledRwLock::new(());
    let reader = lock.read();
    drop(reader);
    lock.release_read();
}
