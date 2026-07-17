#![allow(unexpected_cfgs)]

use mirai_annotations::*;
use std::marker::PhantomData;

pub struct ModeledRwLock<T> {
    _value: PhantomData<T>,
}

pub struct Guard<'a, T> {
    lock: &'a ModeledRwLock<T>,
}

impl<T> ModeledRwLock<T> {
    pub fn new(_value: T) -> Self {
        Self {
            _value: PhantomData,
        }
    }

    pub fn read(&self) -> Guard<'_, T> {
        precondition!(
            get_model_field!(self, writer, 0usize) == 0,
            "read requires no live writer"
        );
        let readers = get_model_field!(self, readers, 0usize);
        precondition!(readers < usize::MAX, "reader count must not overflow");
        set_model_field!(self, readers, readers + 1);
        Guard { lock: self }
    }

    pub fn acquire_write(&self) -> &Self {
        precondition!(
            get_model_field!(self, writer, 0usize) == 0,
            "write requires no live writer"
        );
        precondition!(
            get_model_field!(self, readers, 0usize) == 0,
            "write requires no live readers"
        );
        set_model_field!(self, writer, 1usize);
        self
    }

    pub fn release_read(&self) {
        let readers = get_model_field!(self, readers, 0usize);
        precondition!(readers > 0, "read release requires a live reader");
        set_model_field!(self, readers, readers - 1);
    }
}

impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        self.lock.release_read();
    }
}
