//! This module is pretty bad
//! because the Rc might get sent
//! here

use alloc::rc::Rc;
use core::{
    marker::PhantomData,
    task::{RawWaker, RawWakerVTable, Waker},
};

fn raw_waker<W: RcWake + 'static>(w: Rc<W>) -> RawWaker {
    unsafe fn get_waker_rc<W: RcWake + 'static>(data: *const ()) -> Rc<W> {
        Rc::from_raw(data as *const W)
    }

    unsafe fn waker_clone<W: RcWake + 'static>(data: *const ()) -> RawWaker {
        Rc::increment_strong_count(data);
        waker_new(data as *const W)
    }

    unsafe fn waker_wake<W: RcWake + 'static>(data: *const ()) {
        get_waker_rc::<W>(data).rc_wake();
    }

    unsafe fn waker_wake_by_ref<W: RcWake + 'static>(data: *const ()) {
        let w = get_waker_rc::<W>(data);
        w.rc_wake_by_ref();
        Rc::into_raw(w); // Leak waker
    }

    unsafe fn waker_drop<W: RcWake + 'static>(data: *const ()) {
        get_waker_rc::<W>(data);
    }

    unsafe fn waker_new<W: RcWake + 'static>(data: *const W) -> RawWaker {
        RawWaker::new(
            data as *const (),
            &RawWakerVTable::new(
                waker_clone::<W>,
                waker_wake::<W>,
                waker_wake_by_ref::<W>,
                waker_drop::<W>,
            ),
        )
    }

    unsafe { waker_new(Rc::into_raw(w)) }
}

pub trait RcWake {
    fn rc_wake(self: Rc<Self>) {
        RcWake::rc_wake_by_ref(&self)
    }
    fn rc_wake_by_ref(self: &Rc<Self>);
}

pub trait RcWakeInto {
    fn into_waker(self) -> WakerWrapper;
}

impl<T: RcWake + 'static> RcWakeInto for Rc<T> {
    fn into_waker(self) -> WakerWrapper {
        unsafe { Waker::from_raw(raw_waker(self)) }.into()
    }
}

pub struct WakerWrapper {
    pub waker: Waker,
    marker: PhantomData<&'static core::cell::UnsafeCell<()>>,
}

impl From<Waker> for WakerWrapper {
    fn from(other: Waker) -> WakerWrapper {
        WakerWrapper {
            waker: other,
            marker: PhantomData,
        }
    }
}
