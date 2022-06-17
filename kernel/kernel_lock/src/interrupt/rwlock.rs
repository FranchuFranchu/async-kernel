use kernel_cpu::in_interrupt_context;
use lock_api::{GuardSend, RawRwLock};

pub use super::super::spin::RawRwLock as RawSpinRwLock;

pub struct RawInterruptRwLock {
    internal: RawSpinRwLock,
}

unsafe impl RawRwLock for RawInterruptRwLock {
    type GuardMarker = GuardSend;

    const INIT: RawInterruptRwLock = Self {
        internal: RawSpinRwLock::INIT,
    };

    fn lock_shared(&self) {
        assert!(in_interrupt_context());
        self.internal.lock_shared()
    }

    fn try_lock_shared(&self) -> bool {
        self.internal.try_lock_shared()
    }

    unsafe fn unlock_shared(&self) {
        self.internal.unlock_shared()
    }

    fn lock_exclusive(&self) {
        assert!(in_interrupt_context());
        self.internal.lock_shared()
    }

    fn try_lock_exclusive(&self) -> bool {
        self.internal.try_lock_exclusive()
    }

    unsafe fn unlock_exclusive(&self) {
        assert!(in_interrupt_context());
        self.internal.unlock_exclusive()
    }
}

pub type RwLock<T> = lock_api::RwLock<RawInterruptRwLock, T>;
pub type RwLockReadGuard<'a, T> = lock_api::RwLockReadGuard<'a, RawInterruptRwLock, T>;
pub type RwLockWriteGuard<'a, T> = lock_api::RwLockWriteGuard<'a, RawInterruptRwLock, T>;
