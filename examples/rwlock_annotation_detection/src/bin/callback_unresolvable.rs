#![allow(unexpected_cfgs)]

use mirai_annotations::abstract_value;
use rwlock_annotation_detection::ModeledRwLock;

fn known(lock: &ModeledRwLock<()>) {
    std::hint::black_box(lock);
}

fn other(lock: &ModeledRwLock<()>) {
    std::hint::black_box((lock, 1usize));
}

fn invoke_generic<F>(lock: &ModeledRwLock<()>, callback: F)
where
    F: FnOnce(&ModeledRwLock<()>),
{
    callback(lock);
}

fn main() {
    let first = ModeledRwLock::new(());
    invoke_generic(&first, known as fn(&ModeledRwLock<()>));

    let second = ModeledRwLock::new(());
    let callback = if abstract_value!(true) {
        known as fn(&ModeledRwLock<()>)
    } else {
        other as fn(&ModeledRwLock<()>)
    };
    invoke_generic(&second, callback);
}
