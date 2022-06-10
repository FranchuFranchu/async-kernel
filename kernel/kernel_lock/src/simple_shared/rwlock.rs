use kernel_cpu::load_hartid;
use lock_api::{GuardNoSend, RawRwLock};

pub use super::super::spin::RawRwLock as RawSpinRwLock;
use crate::shared::{lock_and_disable_interrupts, unlock_and_enable_interrupts_if_necessary};

pub struct RawSharedRwLock {
    internal: RawSpinRwLock,
}

unsafe impl RawRwLock for RawSharedRwLock {
    const INIT: RawSharedRwLock = Self {
        internal: RawSpinRwLock::INIT,
    };

    type GuardMarker = GuardNoSend;

    fn lock_shared(&self) {
        debug!(
            "{} {:x} Lock shared",
            load_hartid(),
            (self as *const Self as usize) & 0xffffffff
        );
        lock_and_disable_interrupts();
        self.internal.lock_shared()
    }

    fn try_lock_shared(&self) -> bool {
        if self.internal.try_lock_shared() {
            lock_and_disable_interrupts();
            true
        } else {
            false
        }
    }

    unsafe fn unlock_shared(&self) {
        debug!(
            "{} {:x} Unlock shared",
            load_hartid(),
            (self as *const Self as usize) & 0xffffffff
        );
        self.internal.unlock_shared();
        unlock_and_enable_interrupts_if_necessary();
    }

    fn lock_exclusive(&self) {
        debug!(
            "{} {:x} Lock exclusive {}",
            load_hartid(),
            (self as *const Self as usize) & 0xffffffff,
            self.internal.is_locked()
        );
        lock_and_disable_interrupts();
        self.internal.lock_exclusive()
    }

    fn try_lock_exclusive(&self) -> bool {
        if self.internal.try_lock_exclusive() {
            lock_and_disable_interrupts();
            true
        } else {
            false
        }
    }

    unsafe fn unlock_exclusive(&self) {
        debug!(
            "{} {:x} Unlock exclusive",
            load_hartid(),
            (self as *const Self as usize) & 0xffffffff
        );
        self.internal.unlock_exclusive();
        unlock_and_enable_interrupts_if_necessary();
    }
}

pub type RwLock<T> = lock_api::RwLock<RawSharedRwLock, T>;
pub type RwLockReadGuard<'a, T> = lock_api::RwLockReadGuard<'a, RawSharedRwLock, T>;
pub type RwLockWriteGuard<'a, T> = lock_api::RwLockWriteGuard<'a, RawSharedRwLock, T>;
