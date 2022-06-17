use alloc::boxed::Box;

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
        address: 0x1000_0000 as _,
    }
}

#[no_mangle]
fn get_printer() -> Box<dyn core::fmt::Write> {
    Box::new(get_uart())
}
#[no_mangle]
fn get_printer_lockfree() -> Box<dyn core::fmt::Write> {
    Box::new(get_uart())
}
