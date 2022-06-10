use core::sync::atomic::{AtomicUsize, Ordering};

use lock_api::{GuardSend, RawRwLock};

const SHARED: usize = 1 << 1;
const WRITER: usize = 1 << 0;

pub struct RawSpinRwLock {
    value: AtomicUsize,
}

unsafe impl RawRwLock for RawSpinRwLock {
    const INIT: RawSpinRwLock = Self {
        value: AtomicUsize::new(0),
    };

    type GuardMarker = GuardSend;

    fn lock_shared(&self) {
        while !self.try_lock_shared() {}
    }

    fn try_lock_shared(&self) -> bool {
        let mut outdated_value = self.value.load(Ordering::SeqCst);
        if outdated_value & WRITER != 0 {
            return false;
        }

        while let Err(_e) = self.value.compare_exchange(
            outdated_value,
            outdated_value + SHARED,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            outdated_value = self.value.load(Ordering::SeqCst);
            if outdated_value & WRITER != 0 {
                return false;
            }
        }
        true
    }

    unsafe fn unlock_shared(&self) {
        self.value.fetch_sub(SHARED, Ordering::SeqCst);
    }

    fn lock_exclusive(&self) {
        while !self.try_lock_exclusive() {}
    }

    fn try_lock_exclusive(&self) -> bool {
        let mut outdated_value = self.value.load(Ordering::SeqCst);
        if outdated_value != 0 {
            return false;
        }

        while let Err(_e) = self.value.compare_exchange(
            outdated_value,
            outdated_value + WRITER,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            outdated_value = self.value.load(Ordering::SeqCst);
            if outdated_value != 0 {
                return false;
            }
        }
        true
    }

    unsafe fn unlock_exclusive(&self) {
        self.value.fetch_sub(WRITER, Ordering::SeqCst);
    }

    fn is_locked(&self) -> bool {
        self.value.load(Ordering::SeqCst) != 0
    }
}

pub type RwLock<T> = lock_api::RwLock<RawSpinRwLock, T>;
pub type RwLockReadGuard<'a, T> = lock_api::RwLockReadGuard<'a, RawSpinRwLock, T>;
pub type RwLockWriteGuard<'a, T> = lock_api::RwLockWriteGuard<'a, RawSpinRwLock, T>;
