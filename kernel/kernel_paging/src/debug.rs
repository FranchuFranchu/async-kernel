pub struct Uart {
    address: *mut u8,
}

impl core::fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for i in s.bytes() {
            unsafe { self.address.write(i) }
        }
        Ok(())
    }
}

pub fn get_uart() -> Uart {
    Uart {
        address: if kernel_cpu::read_satp() == 0 { 0x1000_0000 } else { 0x1000_0000u64 + 0xffff_ffc0_0000_0000u64} as _,
    }
}

#[macro_export]
macro_rules! print
{
    ($($args:tt)+) => (#[allow(unused_unsafe)] {
            // Lock the output to prevent lines mixing between each other
            use core::fmt::Write;
            //let l = crate::std_macros::OUTPUT_LOCK.lock();
            let _ = write!(crate::debug::get_uart(), $($args)+);
            });
}

#[macro_export]
macro_rules! println
{
    () => ({
           crate::print!("\r\n")
           });
    ($fmt:expr) => ({
            crate::print!(concat!($fmt, "\r\n"))
            });
    ($fmt:expr, $($args:tt)+) => ({
            crate::print!(concat!($fmt, "\r\n"), $($args)+)
            });
}
