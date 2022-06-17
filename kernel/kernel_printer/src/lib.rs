#![no_std]

extern crate alloc;

use alloc::boxed::Box;

extern "Rust" {
    pub fn get_printer() -> Box<dyn core::fmt::Write>;
    pub fn get_printer_lockfree() -> Box<dyn core::fmt::Write>;
}

#[macro_export]
macro_rules! print
{
    ($($args:tt)+) => (#[allow(unused_unsafe)] {
            // Lock the output to prevent lines mixing between each other
            use core::fmt::Write;
            //let l = crate::std_macros::OUTPUT_LOCK.lock();
            let _ = write!(unsafe { kernel_printer::get_printer() }, $($args)+);
            });
}
#[macro_export]
macro_rules! println
{
    () => ({
           kernel_printer::print!("\r\n")
           });
    ($fmt:expr) => ({
            kernel_printer::print!(concat!($fmt, "\r\n"))
            });
    ($fmt:expr, $($args:tt)+) => ({
            kernel_printer::print!(concat!($fmt, "\r\n"), $($args)+)
            });
}

#[macro_export]
macro_rules! print_u
{
    ($($args:tt)+) => (#[allow(unused_unsafe)] {
            // Lock the output to prevent lines mixing between each other
            use core::fmt::Write;
            //let l = crate::std_macros::OUTPUT_LOCK.lock();
            let _ = write!(unsafe { kernel_printer::get_printer_lockfree() }, $($args)+);
            });
}
#[macro_export]
macro_rules! println_u
{
    () => ({
           kernel_printer::print_u!("\r\n")
           });
    ($fmt:expr) => ({
            kernel_printer::print_u!(concat!($fmt, "\r\n"))
            });
    ($fmt:expr, $($args:tt)+) => ({
            kernel_printer::print_u!(concat!($fmt, "\r\n"), $($args)+)
            });
}
