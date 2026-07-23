#![allow(unexpected_cfgs)]

use mirai_annotations::*;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

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

pub struct Metadata;

impl Metadata {
    pub fn require_ready(&self) {
        precondition!(
            get_model_field!(self, ready, 0usize) == 1,
            "metadata must be ready"
        );
    }
}

pub struct DescriptorTable {
    metadata: Metadata,
}

pub struct DescriptorOwner;

pub struct DescriptorTableGuard<'a> {
    owner: &'a DescriptorOwner,
    table: DescriptorTable,
}

impl DescriptorOwner {
    pub fn descriptor_table_mut(&self) -> DescriptorTableGuard<'_> {
        precondition!(
            get_model_field!(self, write_held, 0usize) == 0,
            "write requires no live writer"
        );
        set_model_field!(self, write_held, 1usize);
        DescriptorTableGuard {
            owner: self,
            table: DescriptorTable::new(),
        }
    }

    pub fn descriptor_table_mut_if(&self, acquire: bool) -> DescriptorTableGuard<'_> {
        set_model_field!(self, write_held, acquire as usize);
        DescriptorTableGuard {
            owner: self,
            table: DescriptorTable::new(),
        }
    }

    pub fn read(&self) {
        precondition!(
            get_model_field!(self, write_held, 0usize) == 0,
            "read requires no live writer"
        );
    }
}

impl DescriptorTable {
    pub fn new() -> Self {
        Self { metadata: Metadata }
    }

    pub fn with_metadata_mut<R>(&mut self, callback: impl FnOnce(&mut Metadata) -> R) -> R {
        callback(&mut self.metadata)
    }

    pub fn with_ready_metadata_mut<R>(&mut self, callback: impl FnOnce(&mut Metadata) -> R) -> R {
        set_model_field!(&self.metadata, ready, 1usize);
        callback(&mut self.metadata)
    }
}

impl Default for DescriptorTable {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for DescriptorTableGuard<'_> {
    type Target = DescriptorTable;

    fn deref(&self) -> &Self::Target {
        &self.table
    }
}

impl DerefMut for DescriptorTableGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.table
    }
}

impl Drop for DescriptorTableGuard<'_> {
    fn drop(&mut self) {
        set_model_field!(self.owner, write_held, 0usize);
    }
}
