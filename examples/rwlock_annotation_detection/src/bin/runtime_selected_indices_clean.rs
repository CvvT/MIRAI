#![allow(unexpected_cfgs)]

use mirai_annotations::{abstract_value, assume};
use rwlock_annotation_detection::ModeledRwLock;

fn select<'a>(locks: &'a [ModeledRwLock<()>; 2], index: usize) -> &'a ModeledRwLock<()> {
    &locks[index]
}

fn main() {
    let locks = [ModeledRwLock::new(()), ModeledRwLock::new(())];
    let first_index = abstract_value!(0usize) % locks.len();
    let second_index = abstract_value!(1usize) % locks.len();
    assume!(first_index != second_index);
    let first = select(&locks, first_index);
    let second = select(&locks, second_index);

    let reader = first.read();
    let writer = second.write();
    std::hint::black_box((&reader, writer));
}
