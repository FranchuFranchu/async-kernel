#![cfg_attr(not(test), no_std)]

use alloc::boxed::Box;
use core::{
    mem::ManuallyDrop,
    ops::Deref,
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

extern crate alloc;

pub struct Arc<T: ?Sized> {
    inner: NonNull<ArcInner<T>>,
}

impl<T: ?Sized> Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner().data
    }
}

impl<T: ?Sized> Arc<T> {
    fn inner(&self) -> &ArcInner<T> {
        unsafe { &self.inner.as_ref() }
    }
}

impl<T> Arc<T> {
    pub fn new(data: T) -> Self {
        let mut inner = ArcInner::new_boxed(data);
        *(unsafe { inner.as_mut() }.strong.get_mut()) += 1;
        Self { inner: inner }
    }
}

#[repr(C)]
pub struct ArcInner<T: ?Sized> {
    strong: AtomicUsize,
    weak: AtomicUsize,
    data: ManuallyDrop<T>,
}
impl<T: ?Sized> ArcInner<T> {
    fn dec_strong_and_drop(&self) {
        if self.strong.fetch_sub(1, Ordering::Relaxed) == 1 {
            unsafe { self.drop_data() }
            if self.weak.load(Ordering::Relaxed) == 0 {
                unsafe { self.drop_self() }
            }
        }
    }
    fn dec_weak_and_drop(&self) {
        if self.weak.fetch_sub(1, Ordering::Relaxed) == 1 {
            if self.strong.load(Ordering::Relaxed) == 0 {
                unsafe { self.drop_data() }
            }
            unsafe { self.drop_self() }
        }
    }
    unsafe fn drop_self(&self) {
        drop(Box::from_raw(self as *const Self as *mut Self))
    }
    unsafe fn drop_data(&self) {
        ManuallyDrop::drop(
            (&self.data as *const ManuallyDrop<T> as *mut ManuallyDrop<T>)
                .as_mut()
                .unwrap(),
        )
    }
}
impl<T> ArcInner<T> {
    fn new(data: T) -> Self {
        Self {
            strong: AtomicUsize::new(0),
            weak: AtomicUsize::new(0),
            data: ManuallyDrop::new(data),
        }
    }
    pub fn new_boxed(data: T) -> NonNull<Self> {
        NonNull::from(Box::leak(Box::new(Self::new(data))))
    }
}

pub struct ArcInject<T: ?Sized, U: ?Sized> {
    inner: NonNull<ArcInner<T>>,
    data: *const U,
}

impl<T: ?Sized, U: ?Sized> ArcInject<T, U> {
    pub fn new(parent: &Arc<T>, get_child: impl FnOnce(&T) -> &U) -> Self {
        parent.inner().strong.fetch_add(1, Ordering::Relaxed);
        Self {
            inner: parent.inner,
            data: get_child(&**parent),
        }
    }

    pub fn new_std(parent: &alloc::sync::Arc<T>, get_child: impl FnOnce(&T) -> &U) -> Self {
        let inner: NonNull<ArcInner<T>> =
            unsafe { *(parent as *const _ as *const NonNull<ArcInner<T>>) };
        unsafe { inner.as_ref() }
            .strong
            .fetch_add(1, Ordering::Relaxed);
        Self {
            inner: inner,
            data: get_child(&**parent),
        }
    }

    fn inner(&self) -> &ArcInner<T> {
        unsafe { &self.inner.as_ref() }
    }

    fn inner_data(inner: NonNull<ArcInner<T>>, data: *const U) -> Self {
        Self { inner, data }
    }

    pub fn downgrade(this: &Self) -> WeakInject<T, U> {
        WeakInject::from(this)
    }

    pub fn uninject_alloc(this: Self) -> Arc<T> {
        unsafe { core::mem::transmute_copy(&this) }
    }

    pub fn deref_inner(this: &Self) -> &T {
        this.inner().data.deref()
    }
}

impl<T: ?Sized, U: ?Sized> Deref for ArcInject<T, U> {
    type Target = U;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.data }
    }
}

impl<T: ?Sized, U: ?Sized> Drop for ArcInject<T, U> {
    fn drop(&mut self) {
        self.inner().dec_strong_and_drop();
    }
}

pub struct WeakInject<T: ?Sized, U: ?Sized> {
    inner: NonNull<ArcInner<T>>,
    data: *const U,
}

unsafe impl<T: ?Sized + Send + Sync, U: ?Sized + Send + Sync> Send for WeakInject<T, U> {}
unsafe impl<T: ?Sized + Send + Sync, U: ?Sized + Send + Sync> Sync for WeakInject<T, U> {}
unsafe impl<T: ?Sized + Send + Sync, U: ?Sized + Send + Sync> Send for ArcInject<T, U> {}
unsafe impl<T: ?Sized + Send + Sync, U: ?Sized + Send + Sync> Sync for ArcInject<T, U> {}

impl<T: ?Sized, U: ?Sized> WeakInject<T, U> {
    fn inner_data(inner: NonNull<ArcInner<T>>, data: *const U) -> Self {
        unsafe { inner.as_ref().weak.fetch_add(1, Ordering::Relaxed) };
        Self { inner, data }
    }

    fn inner(&self) -> &ArcInner<T> {
        unsafe { &self.inner.as_ref() }
    }

    pub fn upgrade(&self) -> Option<ArcInject<T, U>> {
        let inner = self.inner();

        let mut n = inner.strong.load(Ordering::Relaxed);

        loop {
            if n == 0 {
                return None;
            }

            if n > (isize::MAX) as usize {}

            match inner
                .strong
                .compare_exchange_weak(n, n + 1, Ordering::Acquire, Ordering::Relaxed)
            {
                Ok(_) => return Some(ArcInject::inner_data(self.inner, self.data)),
                Err(old) => n = old,
            }
        }
    }
}

impl<T: ?Sized, U: ?Sized> From<&ArcInject<T, U>> for WeakInject<T, U> {
    fn from(this: &ArcInject<T, U>) -> Self {
        Self::inner_data(this.inner, this.data)
    }
}

impl<T: ?Sized, U: ?Sized> Drop for WeakInject<T, U> {
    fn drop(&mut self) {
        self.inner().dec_weak_and_drop()
    }
}
impl<T: ?Sized, U: ?Sized> Clone for WeakInject<T, U> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            data: self.data.clone(),
        }
    }
}

impl<T: ?Sized, U: ?Sized> Clone for ArcInject<T, U> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            data: self.data.clone(),
        }
    }
}

#[test]
fn test() {
    #[derive(Debug)]
    struct Test {
        b: u8,
    }

    let t = Test { b: 1 };
    let ta = Arc::new(t);
    let tb = ArcInject::new(&ta, |t| &t.b);
    println!("{:?}", *tb);
    println!("{:?}", *ta);
}
