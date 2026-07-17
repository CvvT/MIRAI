use rwlock_annotation_detection::ModeledRwLock;
use std::rc::Rc;

fn main() {
    let first = Rc::new(ModeledRwLock::new(()));
    let alias = Rc::clone(&first);

    let first_writer = first.acquire_write();
    let second_writer = alias.acquire_write();
    std::hint::black_box((first_writer, second_writer));
}
