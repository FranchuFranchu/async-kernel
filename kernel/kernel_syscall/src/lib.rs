#![no_std]

extern crate alloc;
#[macro_use]
extern crate kernel_printer;

mod syscall;
use syscall::do_syscall;
