#![allow(unexpected_cfgs)]

use mirai_annotations::set_model_field;
use rwlock_annotation_detection::ModeledRwLock;

fn main() {
    let a = ModeledRwLock::new(());
    let b = ModeledRwLock::new(());

    set_model_field!(&a, writer, 1usize);
    std::hint::black_box(&b);
    let second = a.acquire_write();
    std::hint::black_box(second);
}
