#![no_std]
#![feature(never_type, const_fn_fn_ptr_basics, waker_getters)]

extern crate alloc;

mod executor;
mod never_waker;

pub use executor::{run_neverending_future, run_in_parallel, Executor, ExecutorFuture, ExecutorHandle, RawPtrExecutorHandle};

#[macro_export]
macro_rules! run_in_parallel {
    {$($e:expr)*} => {
        kernel_executor::run_in_parallel([
        	$(alloc::boxed::Box::new(alloc::boxed::Box::pin($e)) as alloc::boxed::Box<kernel_executor::ExecutorFuture>),*
        ])
    };
}