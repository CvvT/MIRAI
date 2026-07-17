use rwlock_annotation_detection::ModeledRwLock;

fn invoke_in_loop<F>(lock: &ModeledRwLock<()>, callback: F)
where
    F: Fn(&ModeledRwLock<()>),
{
    for _ in 0..2 {
        callback(lock);
    }
}

fn main() {
    let first = ModeledRwLock::new(());
    invoke_in_loop(&first, |lock| {
        std::hint::black_box(lock);
    });
    let second = ModeledRwLock::new(());
    invoke_in_loop(&second, |lock| {
        std::hint::black_box((lock, 1usize));
    });
}
