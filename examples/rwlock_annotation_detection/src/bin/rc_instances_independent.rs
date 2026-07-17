use rwlock_annotation_detection::ModeledRwLock;
use std::rc::Rc;

fn main() {
    let first = Rc::new(ModeledRwLock::new(()));
    let second = Rc::new(ModeledRwLock::new(()));

    let first_writer = first.acquire_write();
    let second_writer = second.acquire_write();
    std::hint::black_box((first_writer, second_writer));
}
