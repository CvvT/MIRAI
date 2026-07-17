#![allow(unexpected_cfgs)]

use mirai_annotations::set_model_field;
use rwlock_annotation_detection::ModeledRwLock;

fn main() {
    let a = ModeledRwLock::new(());
    let b = ModeledRwLock::new(());

    set_model_field!(&a, write_held, 1usize);
    let b_reader = b.read();
    std::hint::black_box((&a, &b_reader));
}
