#![no_std]
#![no_main]
#![feature(
    never_type,
    const_fn_fn_ptr_basics,
    panic_info_message,
    const_fn_trait_bound,
    waker_getters,
    default_alloc_error_handler,
    naked_functions,
    asm_sym,
    asm_const
)]

extern crate alloc;

// Use the allocator!
extern crate kernel_allocator;
#[macro_use]
extern crate kernel_printer;

use alloc::{boxed::Box, collections::VecDeque, rc::Rc};
use core::{
    cell::RefCell,
    ffi::c_void,
    sync::atomic::{AtomicUsize, Ordering},
};

use kernel_cpu::{
    csr::status, read_satp, read_sie, read_sscratch, read_sstatus, write_sie, write_sscratch,
    write_sstatus, write_stvec,
};
use kernel_executor::{LocalExecutor, SendExecutor, SendExecutorHandle};
use kernel_process::Process;
use kernel_trap_frame::TrapFrame;
use kernel_util::boxed_slice_with_alignment_uninit;
use local_notify::Notify;
use sbi::SbiError;

use crate::timer::set_relative_timer;

pub mod asm;
pub mod local_notify;
pub mod never_waker;
pub mod std_macros;
pub mod timer;
pub mod trap_handler;
pub mod wait_future;

// Linker symbols
extern "C" {
    static _heap_start: c_void;
    static _heap_end: c_void;

    static _stack_start: c_void;
    static _stack_end: c_void;

    fn s_trap_vector();
    fn new_hart();
}

#[derive(Default)]
struct HartLocals {
    local_executor: Option<kernel_executor::LocalExecutorHandle>,
    executor: Option<kernel_executor::SendExecutorHandle>,
    trap_notify: Notify,
    timer_happened_notify: Notify,
    timer_scheduled_notify: Notify,
    timer_queue: RefCell<VecDeque<(u64, Rc<Notify>)>>,
}

impl HartLocals {
    fn current() -> &'static HartLocals {
        unsafe {
            ((*read_sscratch()).hart_locals as *const HartLocals)
                .as_ref()
                .unwrap()
        }
    }
}

fn enable_interrupts() {
    use kernel_cpu::csr::*;
    unsafe { write_sie(STIE | SEIE | SSIE) };
}

fn disable_interrupts() {
    unsafe { write_sie(0) };
}

#[link_section = ".text.init"]
#[naked]
#[no_mangle]
pub unsafe fn boot() {
    kernel_util::useful_asm_fragment!(boot, main);
}

fn setup_hart_trap_frame(hartid: usize, hart_locals: HartLocals) -> Box<TrapFrame> {
    let mut trap_frame = TrapFrame::zeroed();
    trap_frame.hartid = hartid;
    trap_frame.set_interrupt_context();
    trap_frame.hart_locals = Box::leak(Box::new(hart_locals)) as *mut _ as usize;
    Box::new(trap_frame)
}

#[no_mangle]
pub fn rust_oom() {
    loop {}
}

pub async fn setup_hart(hartid: usize) -> ! {
    setup_hart_state_and_metadata(hartid);

    common_hart_code().await
}

pub static SV_BITS: AtomicUsize = AtomicUsize::new(0);
pub static GAP: AtomicUsize = AtomicUsize::new(0);

#[no_mangle]
pub fn main(
    hartid: usize,
    opaque: usize,
    sv_bits: usize,
    kernel_len: usize,
    stack_start: usize,
    hart_entry_point: usize,
) -> ! {
    if SV_BITS.load(Ordering::Acquire) != 0 {
        // This is not the main hart. Go to the hart entry code

        kernel_executor::run_neverending_future(
            alloc::boxed::Box::pin(setup_hart(hartid)),
            kernel_cpu::wfi,
        )
    };
    unsafe { (0x1000_0000 as *mut u8).write_volatile(67) };
    unsafe { (0x1000_0000 as *mut u8).write_volatile(0xa) };

    let start: usize = 0xffffffc08200_0000;
    let end: usize = 0xffffffc08700_0000;
    kernel_allocator::init_from_pointers(start as *const _, end as *const _);
    println!("{:?}", "Reached kernel!");
    println!("{:x}", stack_start);

    SV_BITS.store(sv_bits, Ordering::Release);
    GAP.store(0usize.wrapping_sub(1 << (sv_bits - 1)), Ordering::Release);

    kernel_executor::run_neverending_future(
        alloc::boxed::Box::pin(async_main(hartid, opaque, hart_entry_point)),
        kernel_cpu::wfi,
    )
}

static mut GLOBAL_EXECUTOR: Option<SendExecutorHandle> = None;

fn setup_hart_state_and_metadata(hartid: usize) {
    let local_executor = LocalExecutor::new();
    unsafe {
        write_stvec(s_trap_vector as usize);
        use kernel_cpu::csr::*;
        write_sstatus(read_sstatus() | status::SIE);

        let handle = GLOBAL_EXECUTOR.as_ref().unwrap().clone();

        let mut hart_locals = HartLocals::default();
        hart_locals.executor = Some(handle);
        hart_locals.local_executor = Some(local_executor.borrow_mut().handle());
        let mut frame = setup_hart_trap_frame(hartid, hart_locals);
        frame.pid = 1;

        let stack = boxed_slice_with_alignment_uninit::<u8>(4096, 4096);
        let stack_addr = stack.as_ptr_range().end as usize;
        Box::leak(stack);

        frame.interrupt_stack = stack_addr;
        frame.satp = read_satp();
        frame.kernel_satp = read_satp();

        write_sscratch(Box::leak(frame) as *mut _ as usize);
    }
}

async fn async_main(hartid: usize, _opaque: usize, hart_entry_point: usize) -> ! {
    let executor = SendExecutor::new();
    unsafe { GLOBAL_EXECUTOR.replace(executor.lock().handle()) };

    setup_hart_state_and_metadata(hartid);

    // Spawn all harts
    for hart_id in 0.. {
        let stack = boxed_slice_with_alignment_uninit::<u8>(4096, 4096);
        let stack_addr = stack.as_ptr_range().end as *mut usize;
        let stack_addr = unsafe { stack_addr.offset(-1) };
        unsafe { stack_addr.write(read_satp()) };
        match sbi::hart_state_management::hart_start(
            hart_id,
            hart_entry_point,
            stack_addr as usize - GAP.load(Ordering::Relaxed),
        ) {
            Ok(status) => {}
            Err(SbiError::AlreadyAvailable) => {}
            Err(SbiError::InvalidParameter) => {
                break;
            }
            _ => {
                panic!("{:?}", "Unhandled SBI error when starting hart");
            }
        };
        Box::leak(stack);
    }

    println!("{:?}", "all harts started up!");

    common_hart_code().await
}

async fn common_hart_code() -> ! {
    HartLocals::current()
        .local_executor
        .as_ref()
        .unwrap()
        .spawn(Box::new(Box::pin(async {
            fn test() {
                unsafe { write_sstatus(read_sstatus() | status::SIE) };
                enable_interrupts();

                for i in 0u64.. {
                    i * i;
                }

                use kernel_cpu::{read_sip, write_sip};
                unsafe { write_sip(read_sip() | kernel_cpu::csr::SSIP) }
                loop {}
            }
            let process = Process::new_supervisor(
                |mut process| process.name = Some(alloc::string::String::from("hello world")),
                test,
            );

            loop {
                set_relative_timer(0x1_000_000);
                process.lock().switch_to_and_come_back();
            }
        })));

    let handle = HartLocals::current()
        .local_executor
        .as_ref()
        .unwrap()
        .clone();
    handle.await;

    loop {}
}

#[no_mangle]
fn test_fn() {}

#[no_mangle]
fn syscall_on_interrupt_disabled() {
    loop {}
}

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

    println!(
        "\"{}\" at \x1b[94m{}\x1b[0m",
        message,
        info.location().unwrap()
    );
    loop {}
}
