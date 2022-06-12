#![no_std]
#![no_main]
#![feature(never_type, const_fn_fn_ptr_basics, panic_info_message, const_fn_trait_bound, waker_getters)]
#![feature(default_alloc_error_handler)]

extern crate alloc;

// Use the allocator!
extern crate kernel_allocator;

use core::pin::Pin;
use core::task::Context;
use core::{ffi::c_void};
use core::future::Future;
use kernel_chip_drivers::fdt;
use kernel_chip_drivers::plic::Plic0;
use kernel_cpu::csr::XCAUSE_DESCRIPTION;
use kernel_executor::{ExecutorFuture, run_in_parallel, Executor};
use alloc::boxed::Box;
use kernel_cpu::{write_stvec, write_sie, write_sstatus, read_sstatus, write_sscratch, read_scause, read_sscratch, read_satp};
use kernel_trap_frame::TrapFrame;
use wait_future::WaitForFunctionCallFuture;

#[macro_use]
extern crate kernel_printer;

pub mod never_waker;
pub mod std_macros;

// Linker symbols
extern "C" {
    static _heap_start: c_void;
    static _heap_end: c_void;

    static _stack_start: c_void;
    static _stack_end: c_void;

    fn s_trap_vector();
    fn new_hart();
}

struct HartLocals {
	executor_handle: kernel_executor::ExecutorHandle,
}

impl HartLocals {
	fn current() -> &'static HartLocals {
		unsafe { 
			((*read_sscratch()).hart_locals as *const HartLocals).as_ref().unwrap()
		}
	}
}

#[no_mangle]
pub fn trap_handler(
) {
	TRAP_FUTURE.wake(());
	disable_interrupts()
}

fn enable_interrupts() {
	use kernel_cpu::csr::*;
	unsafe { write_sie(STIE | SEIE | SSIE) };
}

fn disable_interrupts() {
	use kernel_cpu::csr::*;
	unsafe { write_sie(0) };
}
static TRAP_FUTURE: WaitForFunctionCallFuture<()> = WaitForFunctionCallFuture::new(enable_interrupts);

pub mod wait_future;

fn setup_hart_trap_frame(hartid: usize, hart_locals: HartLocals) -> Box<TrapFrame> {
	let mut trap_frame = TrapFrame::zeroed();
	trap_frame.hartid = hartid;
	trap_frame.hart_locals = Box::leak(Box::new(hart_locals)) as *mut _ as usize;
	Box::new(trap_frame)
}

#[no_mangle]
pub fn rust_oom() {
	loop {}
}

#[no_mangle]
pub extern "C" fn main(mut hartid: usize, opaque: usize) -> ! {
	unsafe { (0x1000_0000 as *mut u8).write_volatile(67) };
	unsafe { (0x1000_0000 as *mut u8).write_volatile(0xa) };
	let start: usize = 0x8200_0000;;
	let end: usize = 0x8800_0000;
	kernel_allocator::init_from_pointers(start as *const _, end as *const _);
	println!("{:?}", "Reached kernel!");
	kernel_executor::run_neverending_future(alloc::boxed::Box::pin(async_main(hartid, opaque)))
}

async fn async_main(hartid: usize, opaque: usize) -> ! {
	let mut executor = Executor::new([]);
	unsafe { 
		write_stvec(s_trap_vector as usize);
		use kernel_cpu::csr::*;
		write_sstatus(read_sstatus() | status::SIE);
		
		let handle = executor.handle();
		
		let mut frame = setup_hart_trap_frame(hartid, HartLocals {
			executor_handle: handle,
		});
		frame.pid = 1;
		frame.interrupt_stack = 0x84000000;
		frame.satp = read_satp();
		frame.kernel_satp = read_satp();
		
		write_sscratch(Box::leak(frame) as *mut _ as usize);
	};
	
	
	fdt::init(opaque as _);
	
	let mut plic = Plic0::new_with_addr(
        fdt::root()
            .read()
            .get("soc/plic@")
            .unwrap()
            .unit_address
            .unwrap(),
    );
    /*
    fdt::root().read().pretty(0);
    plic.set_threshold(0);
    plic.set_enabled(10, true);
    plic.set_priority(10, 3);
    unsafe { (0x1000_0001 as *mut u8).write_volatile(1) }
    */
	
	
	kernel_sbi::set_absolute_timer(0).unwrap();
	
	HartLocals::current().executor_handle.spawn(Box::new(Box::pin(
		async {
			TRAP_FUTURE.wait().await;
			
			let cause = read_scause();
			let is_interrupt = cause >> (usize::BITS - 1) != 0;
			let cause = (cause << 1) >> 1;
			
			if cause == kernel_cpu::csr::cause::SUPERVISOR_TIMER {
				kernel_sbi::set_absolute_timer(u64::MAX);
				println!("Timer interrupt");
			} else if cause == kernel_cpu::csr::cause::SUPERVISOR_SOFTWARE {
				use kernel_cpu::{write_sip, read_sip};
				unsafe { write_sip(read_sip() & (!kernel_cpu::csr::SSIP)) }
			} else if cause == kernel_cpu::csr::cause::SUPERVISOR_EXTERNAL {
				println!("{:?}", "external interrupt");
			}
			if !is_interrupt {
				println!("{}.", XCAUSE_DESCRIPTION[cause]);
			}
			enable_interrupts()
		})));
	
	executor.await;
	
	loop {}
}

#[no_mangle]
fn test_fn() {
	
}

#[no_mangle]
fn hart_entry() {
	loop {}
}

#[no_mangle]
fn syscall_on_interrupt_disabled() {
	loop {}
}

pub struct Uart {
	address: *mut u8
}

impl core::fmt::Write for Uart {
	fn write_str(&mut self, s: &str) -> core::fmt::Result {
	    for i in s.bytes() {
	    	unsafe { self.address.write(i) }
	    };
	    Ok(())
	}
}

pub fn get_uart() -> Uart {
	Uart { address: 0x1000_0000 as _ }
}


#[macro_export]
macro_rules! print
{
    ($($args:tt)+) => (#[allow(unused_unsafe)] {
            // Lock the output to prevent lines mixing between each other
            use core::fmt::Write;
            //let l = crate::std_macros::OUTPUT_LOCK.lock();
            let _ = write!(get_uart(), $($args)+);
            });
}


#[macro_export]
macro_rules! println
{
    () => ({
           print!("\r\n")
           });
    ($fmt:expr) => ({
            print!(concat!($fmt, "\r\n"))
            });
    ($fmt:expr, $($args:tt)+) => ({
            print!(concat!($fmt, "\r\n"), $($args)+)
            });
}


#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
	println!("{:?}", "Panic!");
	
    let fnomsg = format_args!("<no message>");
    let message = info.message().unwrap_or(&fnomsg);
    
	println!("\"{}\" at \x1b[94m{}\x1b[0m", message, info.location().unwrap());
    loop {}
}

pub mod asm;
