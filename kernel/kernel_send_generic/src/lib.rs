#![cfg_attr(not(test), no_std)]
#![feature(generic_associated_types)]

extern crate alloc;

use core::ops::{Deref, DerefMut};

pub mod single_threaded;

pub trait RwLocking<T> {
    type ReadGuard<'this, Q>: Deref<Target = Q>
    where
        Self: 'this,
        Q: 'this;
    type WriteGuard<'this, Q>: DerefMut<Target = Q>
    where
        Self: 'this,
        Q: 'this;
    fn new(internal: T) -> Self;
    fn write(&self) -> Self::WriteGuard<'_, T>;
    fn read(&self) -> Self::ReadGuard<'_, T>;
}

pub trait Locking<T> {
    type LockGuard<'this, Q>: DerefMut<Target = Q>
    where
        Self: 'this,
        Q: 'this;
    fn new(internal: T) -> Self;
    fn lock(&self) -> Self::LockGuard<'_, T>;
}

pub struct SimpleLockWrapper<T>(pub T);

impl<T, L: RwLocking<T>> Locking<T> for SimpleLockWrapper<L> {
    type LockGuard<'this, Q>
    where
        L: 'this,
        Q: 'this,
    = <L as RwLocking<T>>::WriteGuard<'this, Q>;

    fn new(internal: T) -> Self {
        Self(L::new(internal))
    }

    fn lock(&self) -> Self::LockGuard<'_, T> {
        L::write(&self.0)
    }
}

pub trait RefcountWeak<T>: Clone + Default {
    type Strong: Refcount<T>;
    fn upgrade(&self) -> Option<Self::Strong>;
}

pub trait Refcount<T>: Clone + Deref<Target = T> {
    type Weak: RefcountWeak<T>;
    fn new(internal: T) -> Self;
    fn downgrade(this: &Self) -> Self::Weak;
}

pub trait ThreadLocality {
    type RwLocking<T>: RwLocking<T>;
    type Locking<T>: Locking<T>;
    type Refcount<T>: Refcount<T>;
    type RefcountWeak<T>: RefcountWeak<T>;
}
