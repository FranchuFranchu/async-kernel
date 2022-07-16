//! A waker that can be used when you're certain that the future will never be
//! Ready


use core::{
    sync::atomic::{AtomicBool, Ordering},
    task::{RawWaker, RawWakerVTable, Waker},
};

struct WakerData(AtomicBool);

unsafe fn waker_clone(a: *const ()) -> RawWaker {
    RawWaker::new(a, &WAKER_VTABLE)
}
unsafe fn wake(a: *const ()) {
    (a as *const AtomicBool)
        .as_ref()
        .unwrap()
        .store(true, Ordering::Release);
}
unsafe fn wake_by_ref(a: *const ()) {
    (a as *const AtomicBool)
        .as_ref()
        .unwrap()
        .store(true, Ordering::Release);
}
unsafe fn drop(_a: *const ()) {}

const fn waker_vtable_null() -> RawWakerVTable {
    RawWakerVTable::new(waker_clone, wake, wake_by_ref, drop)
}

const WAKER_VTABLE: RawWakerVTable = waker_vtable_null();

const fn new_raw_waker(data: *const ()) -> RawWaker {
    RawWaker::new(data, &WAKER_VTABLE)
}

/// # SAFETY
/// Callers must ensure flag lives longer than the return value and all of its
/// clones
pub unsafe fn new_waker(flag: &AtomicBool) -> Waker {
    Waker::from_raw(new_raw_waker(flag as *const _ as *const ()))
}
