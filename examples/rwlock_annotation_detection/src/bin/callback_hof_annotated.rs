use rwlock_annotation_detection::ModeledRwLock;

fn invoke_annotated<F>(lock: &ModeledRwLock<()>, callback: F)
where
    F: FnOnce(&ModeledRwLock<()>),
{
    callback(lock);
}

fn main() {
    let body_effect = ModeledRwLock::new(());
    invoke_annotated(&body_effect, |lock| {
        let writer = lock.acquire_write();
        std::hint::black_box(writer);
    });
    let second = body_effect.acquire_write();
    std::hint::black_box(second);

    let no_body_effect = ModeledRwLock::new(());
    invoke_annotated(&no_body_effect, |_lock| {});
    let first = no_body_effect.acquire_write();
    std::hint::black_box(first);
}
