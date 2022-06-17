#![no_std]
#![feature(never_type, const_fn_fn_ptr_basics, waker_getters)]

extern crate alloc;

#[macro_use]
extern crate kernel_util;

mod executor;
mod never_waker;
mod non_send_executor;
mod non_send_waker;

pub use executor::{
    run_in_parallel, run_neverending_future, Executor, ExecutorFuture, ExecutorHandle,
    RawPtrExecutorHandle,
};
pub use non_send_executor::{LocalExecutor, LocalExecutorHandle};

#[macro_export]
macro_rules! run_in_parallel {
    {$($e:expr)*} => {
        kernel_executor::run_in_parallel([
        	$(alloc::boxed::Box::new(alloc::boxed::Box::pin($e)) as alloc::boxed::Box<kernel_executor::ExecutorFuture>),*
        ])
    };
}
