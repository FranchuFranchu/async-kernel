use alloc::sync::{Arc, Weak as WeakArc};
use core::ops::Deref;

use kernel_send_generic::{Locking, Refcount, RefcountWeak, RwLocking, ThreadLocality};

struct AtomicLocking<T>(crate::spin::Mutex<T>);

impl<T> Locking<T> for AtomicLocking<T> {
    type LockGuard<'a, Q>
    where
        Q: 'a,
        T: 'a,
    = crate::spin::MutexGuard<'a, Q>;

    fn new(internal: T) -> Self {
        Self(crate::spin::Mutex::new(internal))
    }

    fn lock(&self) -> Self::LockGuard<'_, T> {
        self.0.lock()
    }
}

struct AtomicRwLocking<T>(crate::spin::RwLock<T>);

impl<T> RwLocking<T> for AtomicRwLocking<T> {
    type ReadGuard<'a, Q>
    where
        Q: 'a,
        T: 'a,
    = crate::spin::RwLockReadGuard<'a, Q>;
    type WriteGuard<'a, Q>
    where
        Q: 'a,
        T: 'a,
    = crate::spin::RwLockWriteGuard<'a, Q>;

    fn new(internal: T) -> Self {
        Self(crate::spin::RwLock::new(internal))
    }

    fn write(&self) -> Self::WriteGuard<'_, T> {
        self.0.write()
    }

    fn read(&self) -> Self::ReadGuard<'_, T> {
        self.0.read()
    }
}

struct AtomicRefcount<T>(Arc<T>);

impl<T> Refcount<T> for AtomicRefcount<T> {
    type Weak = AtomicRefcountWeak<T>;

    fn new(internal: T) -> Self {
        Self(Arc::new(internal))
    }

    fn downgrade(this: &Self) -> Self::Weak {
        AtomicRefcountWeak(Arc::downgrade(&this.0))
    }
}

impl<T> Deref for AtomicRefcount<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<T> Clone for AtomicRefcount<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Clone for AtomicRefcountWeak<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Default for AtomicRefcountWeak<T> {
    fn default() -> Self {
        Self(WeakArc::default())
    }
}

struct AtomicRefcountWeak<T>(WeakArc<T>);

impl<T> RefcountWeak<T> for AtomicRefcountWeak<T> {
    type Strong = AtomicRefcount<T>;

    fn upgrade(&self) -> Option<Self::Strong> {
        self.0.upgrade().map(AtomicRefcount)
    }
}

struct AtomicsAndSpinlocks;

impl ThreadLocality for AtomicsAndSpinlocks {
    type Locking<T> = AtomicLocking<T>;
    type Refcount<T> = AtomicRefcount<T>;
    type RwLocking<T> = AtomicRwLocking<T>;
}
