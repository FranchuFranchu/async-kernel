use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
};

use crate::shared::{lock_and_disable_interrupts, unlock_and_enable_interrupts_if_necessary};

pub struct BorrowMut<'mutex, T> {
    mutex: &'mutex RefCell<T>,
}

impl<'mutex, T> Drop for BorrowMut<'mutex, T> {
    fn drop(&mut self) {
        self.mutex.lock.unlock()
    }
}

impl<'mutex, T> Deref for BorrowMut<'mutex, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.mutex.value.get().as_ref().unwrap() }
    }
}
impl<'mutex, T> DerefMut for BorrowMut<'mutex, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.mutex.value.get().as_mut().unwrap() }
    }
}

#[derive(Default)]
struct RawRefCell {
    locked: UnsafeCell<bool>,
}

impl RawRefCell {
    fn lock(&self) {
        lock_and_disable_interrupts();
        let mut ptr = self.locked.get();
        let old = unsafe { ptr.read() };
        if old {
            panic!("Shared RefCell already locked!");
        }
        unsafe { ptr.write(true) };
    }

    fn unlock(&self) {
        let mut ptr = self.locked.get();
        unsafe { ptr.write(false) };
        unlock_and_enable_interrupts_if_necessary();
    }
}

#[derive(Default)]
pub struct RefCell<T> {
    lock: RawRefCell,
    value: UnsafeCell<T>,
}

impl<T> RefCell<T> {
    pub fn new(value: T) -> RefCell<T> {
        Self {
            lock: Default::default(),
            value: UnsafeCell::new(value),
        }
    }

    pub fn borrow_mut(&self) -> BorrowMut<T> {
        self.lock.lock();
        BorrowMut { mutex: self }
    }
}
