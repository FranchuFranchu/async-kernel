use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use kernel_cpu::load_hartid;
use lock_api::{GuardSend, RawMutex};

pub const NO_HART: usize = usize::MAX;

// 1. Define our raw lock type
pub struct RawSpinlock {
    locked: AtomicBool,
    #[cfg(debug_assertions)]
    locker_hartid: AtomicUsize,
}

// 2. Implement RawMutex for this type
unsafe impl RawMutex for RawSpinlock {
    // A spinlock guard can be sent to another thread and unlocked there
    type GuardMarker = GuardSend;

    #[cfg(not(debug_assertions))]
    const INIT: RawSpinlock = RawSpinlock {
        locked: AtomicBool::new(false),
    };
    #[cfg(debug_assertions)]
    const INIT: RawSpinlock = RawSpinlock {
        locked: AtomicBool::new(false),
        locker_hartid: AtomicUsize::new(NO_HART),
    };

    fn lock(&self) {
        // Can fail to lock even if the spinlock is not locked. May be more efficient
        // than `try_lock` when called in a loop.

        #[cfg(debug_assertions)]
        if self.locked.load(Ordering::Acquire)
            && self.locker_hartid.load(Ordering::Acquire) == load_hartid()
        {
            // TODO warn about this somehow
            // warn!("Hart number {} tried locking the same lock twice! (Maybe
            // you're holding a lock a function you're calling needs, or you're
            // waking up a future which uses a lock you're holding)",
            // self.locker_hartid.load(Ordering::Relaxed));
        }
        while !self.try_lock() {
            core::hint::spin_loop()
        }
        //self.locker_hartid.store(load_hartid(), Ordering::Relaxed);
    }

    fn try_lock(&self) -> bool {
        // If self.locked is false, then set it to true and return Ok which gets turned
        // to true in the return value If self.locked is true, then return Err
        // which gets turned to false in the return value
        self.locked
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    unsafe fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

impl RawSpinlock {
    pub unsafe fn unlock_and_swap(&self) -> bool {
        self.locked.swap(false, Ordering::Release)
    }
}

// 3. Export the wrappers. This are the types that your users will actually use.
pub type Mutex<T> = lock_api::Mutex<RawSpinlock, T>;
pub type MutexGuard<'a, T> = lock_api::MutexGuard<'a, RawSpinlock, T>;
