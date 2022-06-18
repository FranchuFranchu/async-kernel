#![no_std]
#![feature(never_type, const_fn_fn_ptr_basics, waker_getters)]

extern crate alloc;

#[macro_use]
extern crate kernel_util;

mod never_waker;
mod non_send_executor;
mod non_send_waker;
mod send_executor;

pub fn run_neverending_future(mut future: impl Future<Output = !> + Unpin, idle: impl Fn()) -> ! {
    let ready_flag = AtomicBool::new(true);
    let waker = unsafe { crate::never_waker::new_waker(&ready_flag) };
    let mut context = Context::from_waker(&waker);
    loop {
        // If the ready flag is true, we can poll it again
        // otherwise, wait a bit.
        if ready_flag.swap(false, core::sync::atomic::Ordering::Relaxed) == true {
            let _ = Pin::new(&mut future).poll(&mut context);
        } else {
            idle()
        }
    }
}

use core::{future::Future, pin::Pin, sync::atomic::AtomicBool, task::Context};

pub use non_send_executor::{LocalExecutor, LocalExecutorHandle};
pub use send_executor::{SendExecutor, SendExecutorHandle};

#[macro_export]
macro_rules! run_in_parallel {
    {$($e:expr)*} => {
        kernel_executor::run_in_parallel([
        	$(alloc::boxed::Box::new(alloc::boxed::Box::pin($e)) as alloc::boxed::Box<kernel_executor::ExecutorFuture>),*
        ])
    };
}
