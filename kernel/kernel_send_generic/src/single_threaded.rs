use alloc::rc::{Rc, Weak};
use core::{
    cell::{Ref, RefCell, RefMut},
    ops::Deref,
};

use crate::{Locking, Refcount, RefcountWeak, RwLocking, SimpleLockWrapper, ThreadLocality};

struct ThreadRefcount<T>(Rc<T>);

impl<T> Refcount<T> for ThreadRefcount<T> {
    type Weak = ThreadRefcountWeak<T>;

    fn new(internal: T) -> Self {
        Self(Rc::new(internal))
    }

    fn downgrade(this: &Self) -> Self::Weak {
        ThreadRefcountWeak(Rc::downgrade(&this.0))
    }
}

struct ThreadRefcountWeak<T>(Weak<T>);

impl<T> RefcountWeak<T> for ThreadRefcountWeak<T> {
    type Strong = ThreadRefcount<T>;

    fn upgrade(&self) -> Option<Self::Strong> {
        self.0.upgrade().map(ThreadRefcount)
    }
}

struct ThreadRwLocking<T>(RefCell<T>);

impl<T> RwLocking<T> for ThreadRwLocking<T> {
    type ReadGuard<'this, Q>
    where
        Self: 'this,
        T: 'this,
        Q: 'this,
    = Ref<'this, Q>;
    type WriteGuard<'this, Q>
    where
        Self: 'this,
        T: 'this,
        Q: 'this,
    = RefMut<'this, Q>;

    fn new(internal: T) -> Self {
        Self(RefCell::new(internal))
    }

    fn write(&self) -> Self::WriteGuard<'_, T> {
        self.0.borrow_mut()
    }

    fn read(&self) -> Self::ReadGuard<'_, T> {
        self.0.borrow()
    }
}

impl<T> Deref for ThreadRefcount<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<T> Clone for ThreadRefcount<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Clone for ThreadRefcountWeak<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Default for ThreadRefcountWeak<T> {
    fn default() -> Self {
        Self(Weak::default())
    }
}

struct NonAtomic;

impl ThreadLocality for NonAtomic {
    type Locking<T> = SimpleLockWrapper<ThreadRwLocking<T>>;
    type Refcount<T> = ThreadRefcount<T>;
    type RefcountWeak<T> = ThreadRefcountWeak<T>;
    type RwLocking<T> = ThreadRwLocking<T>;
}

#[test]
fn test() {
    fn q<Locality: ThreadLocality>() {
        let b = Locality::Refcount::new(1);
        b.clone();
        b.clone();
        b.clone();
    }
    q::<NonAtomic>();
}
