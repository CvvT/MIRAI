#![allow(unexpected_cfgs)]

use mirai_annotations::*;
use std::marker::PhantomData;

pub struct ModeledRwLock<T> {
    _value: PhantomData<T>,
}

pub struct Guard<'a, T> {
    lock: &'a ModeledRwLock<T>,
}

pub struct WriteGuard<'a, T> {
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
            get_model_field!(self, write_held, 0usize) == 0,
            "read requires no live writer"
        );
        let read_count = get_model_field!(self, read_count, 0usize);
        precondition!(read_count < usize::MAX, "reader count must not overflow");
        set_model_field!(self, read_count, read_count + 1);
        Guard { lock: self }
    }

    pub fn write(&self) -> WriteGuard<'_, T> {
        precondition!(
            get_model_field!(self, write_held, 0usize) == 0,
            "write requires no live writer"
        );
        precondition!(
            get_model_field!(self, read_count, 0usize) == 0,
            "write requires no live readers"
        );
        set_model_field!(self, write_held, 1usize);
        WriteGuard { lock: self }
    }

    // Retained for fixtures that model acquisition without RAII release.
    pub fn acquire_write(&self) -> &Self {
        precondition!(
            get_model_field!(self, write_held, 0usize) == 0,
            "write requires no live writer"
        );
        precondition!(
            get_model_field!(self, read_count, 0usize) == 0,
            "write requires no live readers"
        );
        set_model_field!(self, write_held, 1usize);
        self
    }

    pub fn release_read(&self) {
        let read_count = get_model_field!(self, read_count, 0usize);
        precondition!(read_count > 0, "read release requires a live reader");
        set_model_field!(self, read_count, read_count - 1);
    }
}

impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        self.lock.release_read();
    }
}

impl<T> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        set_model_field!(self.lock, write_held, 0usize);
    }
}
