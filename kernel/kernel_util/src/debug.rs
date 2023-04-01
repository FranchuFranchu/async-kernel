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

impl Uart {
    
    pub fn from_address(address : *mut u8) -> Uart {
        Uart { address }
    }
    
}

pub fn get_uart() -> Uart {
    Uart {
        address: 0xffff_ffc0_1000_0000u64 as _,
    }
}

#[macro_export]
macro_rules! print
{
    ($($args:tt)+) => (#[allow(unused_unsafe)] {
            // Lock the output to prevent lines mixing between each other
            use core::fmt::Write;
            //let l = crate::std_macros::OUTPUT_LOCK.lock();
            let _ = write!(::kernel_util::debug::get_uart(), $($args)+);
            });
}

#[macro_export]
macro_rules! println
{
    () => ({
           ::kernel_util::print!("\r\n")
           });
    ($fmt:expr) => ({
            ::kernel_util::print!(concat!($fmt, "\r\n"))
            });
    ($fmt:expr, $($args:tt)+) => ({
            ::kernel_util::print!(concat!($fmt, "\r\n"), $($args)+)
            });
}
