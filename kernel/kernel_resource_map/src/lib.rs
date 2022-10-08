#![no_std]

use core::option::Option;
use core::marker::PhantomData;
use alloc::vec::Vec;

extern crate alloc;

pub struct ResourceMap<T> {
    map: Vec<Option<T>>
}

#[derive(Copy, Clone)]
pub struct ResourceId<T> { pub index: usize, phantom: PhantomData<T> }

impl<T> ResourceId<T> {
    pub const fn new(index: usize) -> Self {
        Self {
            index,
            phantom: PhantomData,
        }
    }
}

impl<T> ResourceMap<T> {
    pub const fn new() -> Self {
        Self {
            map: Vec::new(),
        }
    }
    pub fn add(&mut self, value: T) -> ResourceId<T> {
        let index = self.map.len();
        self.map.push(Some(value));
        ResourceId {
            index,
            phantom: PhantomData,
        }
    }
    pub fn get(&self, id: ResourceId<T>) -> Option<&T> {
        self.map.get(id.index).map(|s| s.as_ref()).unwrap_or(None)
    }
    pub fn get_mut(&mut self, id: ResourceId<T>) -> Option<&mut T> {
        self.map.get_mut(id.index).map(|s| s.as_mut()).unwrap_or(None)
    }
    pub fn delete(&mut self, id: ResourceId<T>) {
        if let Some(e) = self.map.get_mut(id.index) {
            *e = None
        }
    }
    pub fn take(&mut self, id: ResourceId<T>) -> Option<T> {
        self.map.get_mut(id.index).unwrap_or(&mut None).take()
    }
}

impl<T> Default for ResourceMap<T> {
    fn default() -> Self {
        Self::new()
    }
}