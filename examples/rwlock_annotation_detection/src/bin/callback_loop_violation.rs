#![allow(unexpected_cfgs)]

use rwlock_annotation_detection::ModeledRwLock;

fn read(lock: &ModeledRwLock<()>) {
    let reader = lock.read();
    std::hint::black_box(reader);
}

fn invoke_at_inner_depth<F>(depth: usize, lock: &ModeledRwLock<()>, callback: F)
where
    F: Fn(&ModeledRwLock<()>) + Copy,
{
    if depth == 0 {
        return;
    }
    if depth == 1 {
        let writer = lock.write();
        callback(lock);
        drop(writer);
    }
    invoke_at_inner_depth(depth - 1, lock, callback);
}

fn main() {
    let lock = ModeledRwLock::new(());
    invoke_at_inner_depth(1, &lock, read);
}
