use alloc::vec::Vec;
use core::{
    cell::UnsafeCell,
    future::Future,
    ops::{Deref, DerefMut},
    pin::Pin,
    task::{Context, Poll, Waker},
};

use lock_api::RawMutex;

use crate::{shared::RawMutex as RawSharedMutex, spin::mutex::RawSpinlock};

pub struct MutexLockFuture<'mutex, T> {
    mutex: &'mutex AsyncMutex<T>,
}

struct RawAsyncMutex {
    wakers: crate::shared::Mutex<Vec<Waker>>,
    locked: RawSpinlock,
}

pub struct AsyncMutex<T> {
    lock: RawAsyncMutex,
    value: UnsafeCell<T>,
}

impl RawAsyncMutex {
    const fn new() -> Self {
        Self {
            wakers: crate::shared::Mutex::const_new(RawSharedMutex::INIT, Vec::new()),
            locked: RawSpinlock::INIT,
        }
    }
}

impl<'mutex, T> Future for MutexLockFuture<'mutex, T> {
    type Output = AsyncMutexGuard<'mutex, T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.mutex.lock.locked.try_lock() {
            // Lock successful
            Poll::Ready(AsyncMutexGuard { mutex: self.mutex })
        } else {
            // Locking not successful
            // Register the waker
            self.mutex.lock.wakers.lock().push(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl<T> AsyncMutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
            lock: RawAsyncMutex::new(),
        }
    }

    pub fn lock<'mutex>(&'mutex self) -> MutexLockFuture<'mutex, T> {
        MutexLockFuture { mutex: self }
    }

    unsafe fn force_unlock(&self) {
        self.lock.locked.unlock();
        let waker = self.lock.wakers.lock().remove(0);
        waker.wake();
    }
}

pub struct AsyncMutexGuard<'mutex, T> {
    mutex: &'mutex AsyncMutex<T>,
}

impl<'mutex, T> Deref for AsyncMutexGuard<'mutex, T> {
    type Target = T;

    fn deref(&self) -> &'mutex T {
        unsafe { &*self.mutex.value.get() }
    }
}

impl<'mutex, T> DerefMut for AsyncMutexGuard<'mutex, T> {
    fn deref_mut(&mut self) -> &'mutex mut T {
        unsafe { &mut *self.mutex.value.get() }
    }
}

impl<'mutex, T> Drop for AsyncMutexGuard<'mutex, T> {
    fn drop(&mut self) {
        unsafe { self.mutex.force_unlock() };
    }
}

unsafe impl<T> Send for AsyncMutex<T> {}
unsafe impl<T> Sync for AsyncMutex<T> {}

pub type Mutex<T> = AsyncMutex<T>;
pub type MutexGuard<'mutex, T> = AsyncMutexGuard<'mutex, T>;
