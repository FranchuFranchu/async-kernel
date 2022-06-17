use alloc::vec::Vec;
use core::{
    cell::UnsafeCell,
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    task::{Context, Poll, Waker},
};

use lock_api::{RawMutex, RawRwLock};

use crate::{shared::RawMutex as RawSharedMutex, spin::rwlock::RawSpinRwLock};

pub struct RwLockReadFuture<'rwlock, T> {
    rwlock: &'rwlock AsyncRwLock<T>,
}
pub struct RwLockWriteFuture<'rwlock, T> {
    rwlock: &'rwlock AsyncRwLock<T>,
}

struct RawAsyncRwLock {
    wakers: crate::shared::Mutex<Vec<Waker>>,
    locked: RawSpinRwLock,
}

pub struct AsyncRwLock<T> {
    lock: RawAsyncRwLock,
    value: UnsafeCell<T>,
}

impl RawAsyncRwLock {
    const fn new() -> Self {
        Self {
            wakers: crate::shared::Mutex::const_new(RawSharedMutex::INIT, Vec::new()),
            locked: RawSpinRwLock::INIT,
        }
    }
}

impl<'rwlock, T: 'rwlock> Future for RwLockReadFuture<'rwlock, T> {
    type Output = AsyncRwLockReadGuard<'rwlock, T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.rwlock.lock.locked.try_lock_shared() {
            // Lock successful
            Poll::Ready(AsyncRwLockReadGuard {
                rwlock: self.rwlock,
            })
        } else {
            // Locking not successful
            // Register the waker
            self.rwlock.lock.wakers.lock().push(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl<'rwlock, T: 'rwlock> Future for RwLockWriteFuture<'rwlock, T> {
    type Output = AsyncRwLockWriteGuard<'rwlock, T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.rwlock.lock.locked.try_lock_exclusive() {
            // Lock successful
            Poll::Ready(AsyncRwLockWriteGuard {
                rwlock: self.rwlock,
            })
        } else {
            // Locking not successful
            // Register the waker
            self.rwlock.lock.wakers.lock().push(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl<T> AsyncRwLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
            lock: RawAsyncRwLock::new(),
        }
    }

    pub fn read<'rwlock>(&'rwlock self) -> RwLockReadFuture<'rwlock, T> {
        RwLockReadFuture { rwlock: self }
    }

    pub fn write<'rwlock>(&'rwlock self) -> RwLockWriteFuture<'rwlock, T> {
        RwLockWriteFuture { rwlock: self }
    }

    unsafe fn force_unlock_exclusive(&self) {
        self.lock.locked.unlock_exclusive();
        for waker in self.lock.wakers.lock().iter() {
            waker.wake_by_ref();
        }
        self.lock.wakers.lock().clear();
    }

    unsafe fn force_unlock_shared(&self) {
        self.lock.locked.unlock_shared();

        for waker in self.lock.wakers.lock().iter() {
            waker.wake_by_ref();
        }
        self.lock.wakers.lock().clear();
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }

    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

pub struct AsyncRwLockWriteGuard<'rwlock, T> {
    rwlock: &'rwlock AsyncRwLock<T>,
}

impl<'rwlock, T> Deref for AsyncRwLockWriteGuard<'rwlock, T> {
    type Target = T;

    fn deref(&self) -> &'rwlock T {
        unsafe { &*self.rwlock.value.get() }
    }
}

impl<'rwlock, T> DerefMut for AsyncRwLockWriteGuard<'rwlock, T> {
    fn deref_mut(&mut self) -> &'rwlock mut T {
        unsafe { &mut *self.rwlock.value.get() }
    }
}

impl<'rwlock, T> Drop for AsyncRwLockWriteGuard<'rwlock, T> {
    fn drop(&mut self) {
        unsafe { self.rwlock.force_unlock_exclusive() };
    }
}

pub struct AsyncRwLockReadGuard<'rwlock, T> {
    rwlock: &'rwlock AsyncRwLock<T>,
}

impl<'rwlock, T> Deref for AsyncRwLockReadGuard<'rwlock, T> {
    type Target = T;

    fn deref(&self) -> &'rwlock T {
        unsafe { &*self.rwlock.value.get() }
    }
}

impl<'rwlock, T> Drop for AsyncRwLockReadGuard<'rwlock, T> {
    fn drop(&mut self) {
        unsafe { self.rwlock.force_unlock_shared() };
    }
}

impl<T> Default for AsyncRwLock<T>
where
    T: Default,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

unsafe impl<T> Send for AsyncRwLock<T> {}
unsafe impl<T> Sync for AsyncRwLock<T> {}
unsafe impl<'rwlock, T> Send for AsyncRwLockReadGuard<'rwlock, T> {}
unsafe impl<'rwlock, T> Send for AsyncRwLockWriteGuard<'rwlock, T> {}

pub type RwLock<T> = AsyncRwLock<T>;
pub type RwLockReadGuard<'rwlock, T> = AsyncRwLockReadGuard<'rwlock, T>;
pub type RwLockWriteGuard<'rwlock, T> = AsyncRwLockWriteGuard<'rwlock, T>;
