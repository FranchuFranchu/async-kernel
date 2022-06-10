use core::sync::atomic::{AtomicUsize, Ordering};

use lock_api::{GuardNoSend, RawMutex};

pub use super::super::spin::RawMutex as RawSpinlock;

pub const NO_HART: usize = usize::MAX;

// 1. Define our raw lock type
pub struct RawSharedLock {
    internal: RawSpinlock,
    old_sie: AtomicUsize,
}

// 2. Implement RawMutex for this type
unsafe impl RawMutex for RawSharedLock {
    const INIT: RawSharedLock = RawSharedLock {
        internal: RawSpinlock::INIT,
        old_sie: AtomicUsize::new(0),
    };

    // A spinlock guard can be sent to another thread and unlocked there
    type GuardMarker = GuardNoSend;

    fn lock(&self) {
        while !self.try_lock() {}
    }

    fn try_lock(&self) -> bool {
        if self.internal.try_lock() {
            self.old_sie.store(kernel_cpu::read_sie(), Ordering::SeqCst);
            unsafe {
                kernel_cpu::write_sie(0);
            }
            true
        } else {
            false
        }
    }

    unsafe fn unlock(&self) {
        self.internal.unlock();

        kernel_cpu::write_sie(self.old_sie.load(Ordering::SeqCst));
    }
}

// 3. Export the wrappers. This are the types that your users will actually use.
pub type Mutex<T> = lock_api::Mutex<RawSharedLock, T>;
pub type MutexGuard<'a, T> = lock_api::MutexGuard<'a, RawSharedLock, T>;
