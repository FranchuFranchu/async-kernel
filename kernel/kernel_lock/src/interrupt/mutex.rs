//! Locks that are used exclusively in interrupt contexts
//! Essentially a spinlock

use kernel_cpu::in_interrupt_context;
use lock_api::{GuardSend, RawMutex};

pub use super::super::spin::RawMutex as RawSpinlock;

pub const NO_HART: usize = usize::MAX;

// 1. Define our raw lock type
pub struct RawInterruptLock {
    internal: RawSpinlock,
}

// 2. Implement RawMutex for this type
unsafe impl RawMutex for RawInterruptLock {
    // A spinlock guard can be sent to another thread and unlocked there
    type GuardMarker = GuardSend;

    const INIT: RawInterruptLock = RawInterruptLock {
        internal: RawSpinlock::INIT,
    };

    fn lock(&self) {
        assert!(in_interrupt_context());
        // Can fail to lock even if the spinlock is not locked. May be more efficient
        // than `try_lock` when called in a loop.
        self.internal.lock()
    }

    fn try_lock(&self) -> bool {
        self.internal.try_lock()
    }

    unsafe fn unlock(&self) {
        assert!(in_interrupt_context());
        self.internal.unlock()
    }
}

// 3. Export the wrappers. This are the types that your users will actually use.
pub type Mutex<T> = lock_api::Mutex<RawInterruptLock, T>;
pub type MutexGuard<'a, T> = lock_api::MutexGuard<'a, RawInterruptLock, T>;
