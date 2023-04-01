#![no_std]
#![no_main]
#![feature(
    never_type,
    const_fn_fn_ptr_basics,
    panic_info_message,
    bench_black_box,
    const_fn_trait_bound,
    strict_provenance,
    waker_getters,
    default_alloc_error_handler,
    drain_filter,
    int_log,
    generic_arg_infer,
    naked_functions,
    const_btree_new,
    asm_sym,
    asm_const
)]

extern crate alloc;

// Use the allocator!
extern crate kernel_allocator;
#[macro_use]
extern crate kernel_printer;

use alloc::{
    boxed::Box,
    collections::{BTreeMap, VecDeque},
    rc::Rc,
    vec::Vec,
};
use core::{
    cell::RefCell,
    ffi::c_void,
    hint::black_box,
    sync::atomic::{AtomicUsize, Ordering},
    task::Waker, fmt::Write,
};

use fdt::Fdt;
use kernel_chip_drivers::plic::Plic0;
use kernel_cpu::{
    csr::{
        status::{SIE},
        PagingMode,
    },
    in_interrupt_context, read_satp, read_sie, read_sip, read_sscratch, read_sstatus, read_time,
    write_satp, write_sie, write_sscratch, write_sstatus, write_stvec, read_satp_flags, load_hartid, read_sp,
};
use kernel_executor::{LocalExecutor, SendExecutor, SendExecutorHandle};
use kernel_paging::Paging;
use kernel_process::Process;
use kernel_syscall::do_syscall_and_drop_if_exit;
use kernel_trap_frame::TrapFrame;
use kernel_util::{boxed_slice_with_alignment_uninit, boxed_slice_with_alignment, debug::Uart};
use sbi::SbiError;

use crate::{
    asm::{do_supervisor_syscall_2, do_supervisor_syscall_0, do_supervisor_syscall_1}, syscall::wait_until_process_is_woken, timer::set_relative_timer,
    trap_handler::handle_come_back_from_process,
};

pub mod asm;
pub mod never_waker;
pub mod std_macros;
pub mod syscall;
pub mod timer;
pub mod trap_handler;
pub mod wait_future;

#[macro_use]
extern crate bitmask;

// Linker symbols
extern "C" {
    static _heap_start: c_void;
    static _heap_end: c_void;

    static _stack_start: c_void;
    static _stack_end: c_void;

    static _execute_start: c_void;
    static _execute_end: c_void;
    static _readonly_start: c_void;
    static _readonly_end: c_void;
    static _readwrite_start: c_void;
    static _readwrite_end: c_void;
    static _stack_heap_start: c_void;

    fn s_trap_vector();
    fn new_hart();
}

#[derive(Default)]
struct HartLocals {
    local_executor: Option<kernel_executor::LocalExecutorHandle>,
    executor: Option<kernel_executor::SendExecutorHandle>,
    interrupt_notifiers: RefCell<BTreeMap<usize, Vec<Waker>>>,
    unhandled_interrupts: RefCell<usize>,
}

fn loop_forever_black_box() {
    unsafe { core::arch::asm!("j .") };
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

pub fn busy_wait_for(time: u64) {
    let s_time = read_time();
    while read_time() < (time + s_time) {}
}

#[link_section = ".text.init"]
#[naked]
#[no_mangle]
pub unsafe fn boot() {
    kernel_util::useful_asm_fragment!(boot, main);
}

fn setup_hart_trap_frame(hartid: usize, hart_locals: HartLocals) -> Box<TrapFrame> {
    let mut trap_frame = TrapFrame::zeroed_interrupt_context();
    trap_frame.hartid = hartid;
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
/// The beginning of the kernel image in physical memory
pub static KERNEL_START_PHYSICAL: AtomicUsize = AtomicUsize::new(0);
/// The beginning of the kernel image in virtual memory
pub const KERNEL_START_VIRTUAL: usize = usize::MAX - 0x8000_0000 + 1;

pub fn phys_to_virt(phys_addr: usize) -> usize {
    phys_addr + GAP.load(Ordering::Relaxed)
}

pub fn virt_to_phys(virt_addr: usize) -> usize {
    virt_addr - GAP.load(Ordering::Relaxed)
}

pub unsafe fn do_memory_probe(start: usize, end: usize) {
    println!("Starting memory probe...");
    println!("- Start address: {:x}", start);
    println!("- End address: {:x}", end);
    for i in (start..end).step_by(0x1000) {
        let ptr = (i as *mut u8);
        if i % 0x10000 == 0 {
            println!("Probe 0x{:x}", i);
        }
        ptr.write_volatile(0x55);
        if ptr.read_volatile() != 0x55 {
            println!("{:?}", "nope!");
            loop {
                
            } 
        }
    }
    //core::arch::asm!("unimp");
    println!("Completed memory probe");
}

#[no_mangle]
pub fn main(
    hartid: usize,
    opaque: usize,
    sv_bits: usize,
    kernel_len: usize,
    _stack_start_virtual: usize,
    hart_entry_point: usize,
) -> ! {
    black_box(&debug_test_fn());
    if SV_BITS.load(Ordering::Acquire) != 0 {
        // This is not the main hart. Go to the hart entry code

        kernel_executor::run_neverending_future(
            alloc::boxed::Box::pin(setup_hart(hartid)),
            idle_task,
        )
    };

    SV_BITS.store(sv_bits, Ordering::Release);
    GAP.store(0usize.wrapping_sub(1 << (sv_bits - 1)), Ordering::Release);
    
    kernel_util::debug::Uart::from_address(0x1000_0000 as *mut u8).write_str("Reached early kernel code.\n");

    let kernel_phys = unsafe {
        let mut table = ((read_satp() << 12) as *mut Table<_>).as_mut().unwrap();
        let sv39 = Sv39 {
            table: &mut table,
            phys_to_virt,
            virt_to_phys,
        };
        sv39.query(0xffffffff8000_0000).unwrap()
    };
    KERNEL_START_PHYSICAL.store(kernel_phys, Ordering::Relaxed);
    
    
    let gap = GAP.load(Ordering::Relaxed);
    println!("Physical address of kernel: {:x}", kernel_phys);
    println!("Gap: {:x}", gap);
    println!("Computed virtual address of kernel: {:x}", kernel_phys + gap);
    println!("Kernel length in bytes: 0x{:x}", kernel_len);
    println!("Computer kernel end: {:x}", kernel_phys + gap + kernel_len);
    println!("Stack virtual address: {:x}", _stack_start_virtual);
    println!("Stack pointer: {:x}", read_sp());
    
    // kernel_phys + GAP.load(Ordering::Relaxed);
    // Every time i've tried changing this, it's cursed.
    let start: usize = 0xffffffc08400_0000;
    //assert!(start > kernel_phys + GAP.load(Ordering::Relaxed));
    //assert!(kernel_len < 0x50_0000);
    let end: usize = 0xffffffc08700_0000;
    //unsafe { do_memory_probe(start, end) }
    kernel_allocator::init_from_pointers(start as *const _, end as *const _);

    println!("{:?}", "Reached kernel!");

    println!("{:x}", KERNEL_START_PHYSICAL.load(Ordering::Relaxed));

    kernel_executor::run_neverending_future(
        alloc::boxed::Box::pin(async_main(hartid, opaque, hart_entry_point)),
        idle_task,
    )
}

static mut GLOBAL_EXECUTOR: Option<SendExecutorHandle> = None;

fn idle_task() {
    fn idle_task_fn() {
        loop {
            kernel_cpu::wfi();
        }
    }
    let hart_locals = HartLocals::current();
    let mut unhandled = hart_locals.unhandled_interrupts.borrow_mut();
    let prev_unhandled = *unhandled;
    if read_sip() != 0 {
        // There was an interrupt that went unhandled all the way here.
        *unhandled += 1;
        if *unhandled > 1 {
            handle_come_back_from_process(None);
            *unhandled = 0;
        }
    }
    let process = Process::new_supervisor(
        |mut process| {
            process.name = Some(alloc::string::String::from("Idle task"));
        },
        idle_task_fn,
        phys_to_virt,
        virt_to_phys
    );

    if prev_unhandled == 0 {
        set_relative_timer(0x0100_0000);
        //println!("{:?}", "IDLE --------------");
        process.lock().switch_to_and_come_back();
    }
    assert!(in_interrupt_context());
}

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
        //loop_forever_black_box();
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

use kernel_paging::{EntryBits, Sv39, Table};

async fn async_main(hartid: usize, opaque: usize, hart_entry_point: usize) -> ! {
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
            Ok(_status) => {}
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

    println!(
        "ISA: {}",
        unsafe { Fdt::from_ptr(opaque as _) }
            .unwrap()
            .cpus()
            .next()
            .unwrap()
            .property("riscv,isa")
            .unwrap()
            .as_str()
            .unwrap()
    );

    common_hart_code().await
}

#[no_mangle]
fn debug_test_fn() {
    println!("{:#x}", read_sstatus());
    println!("{:#x}", read_sstatus() & SIE);
}

async fn common_hart_code() -> ! {
    // Create a page table for this hart.
    let mut table = kernel_paging::Table::boxed_zeroed();
    let mut sv39 = Sv39 {
        table: &mut *table,
        phys_to_virt,
        virt_to_phys,
    };
    let kernel_physical = KERNEL_START_PHYSICAL.load(Ordering::Relaxed);
    sv39.map(
        0,
        1 << 38,
        1 << 38,
        EntryBits::VALID | EntryBits::READ | EntryBits::WRITE,
    );
    // Map executable area
    // Turns a kernel virtual address into a physical one
    let kernel_virt_to_phys = |addr| {
        kernel_physical
            .wrapping_add(addr)
            .wrapping_sub(KERNEL_START_VIRTUAL)
    };
    let text_start = unsafe { core::ptr::addr_of!(_execute_start).addr() };
    let text_size = unsafe {
        core::ptr::addr_of!(_execute_end).addr() - core::ptr::addr_of!(_execute_start).addr()
    };
    let ro_start = unsafe { core::ptr::addr_of!(_readonly_start).addr() };
    let ro_size = unsafe {
        core::ptr::addr_of!(_readonly_end).addr() - core::ptr::addr_of!(_readonly_start).addr()
    };
    let rw_start = unsafe { core::ptr::addr_of!(_readwrite_start).addr() };
    let rw_size = unsafe {
        core::ptr::addr_of!(_readwrite_end).addr() - core::ptr::addr_of!(_readwrite_start).addr()
    };
    let rest_start = unsafe { core::ptr::addr_of!(_stack_heap_start).addr() };
    sv39.map(
        kernel_virt_to_phys(text_start),
        text_start,
        text_size,
        EntryBits::VALID | EntryBits::READ | EntryBits::EXECUTE,
    );
    sv39.map(
        kernel_virt_to_phys(ro_start),
        ro_start,
        ro_size,
        EntryBits::VALID | EntryBits::READ,
    );
    sv39.map(
        kernel_virt_to_phys(rw_start),
        rw_start,
        rw_size,
        EntryBits::VALID | EntryBits::READ | EntryBits::WRITE,
    );
    sv39.map(
        kernel_virt_to_phys(rest_start),
        rest_start,
        0x0005_0000,
        EntryBits::VALID | EntryBits::READ | EntryBits::WRITE,
    );
    unsafe { assert!(sv39.query(KERNEL_START_VIRTUAL + 0x1000).unwrap() != 0) };

    unsafe {
        let w = sv39.copy_partial_mapping(KERNEL_START_VIRTUAL, 0x1000000);
        println!("{:?}", w)
    }
    drop(sv39);

    let addr: usize = &*table as *const _ as usize;
    assert!(addr & 4095 == 0);
    let mut satp: usize = virt_to_phys(addr) >> 12;
    satp |= 8 << 60;

    unsafe { write_satp(satp) }
    unsafe { (*read_sscratch()).satp = satp }
    unsafe { (*read_sscratch()).kernel_satp = satp }

    // Create the PLIC instance
    let plic = Plic0::new_with_addr(GAP.load(Ordering::Relaxed) + 0x0c00_0000);

    plic.set_threshold(0);
    plic.set_priority(10, 3);
    unsafe {
        (0x1000_0001 as *mut u8)
            .add(GAP.load(Ordering::Relaxed))
            .write_volatile(1)
    }

    println!("{:?}", "ready plic");
    
    if load_hartid() != 0 {
        HartLocals::current().local_executor.as_ref().unwrap().spawn(Box::new(Box::pin(async {
            
            assert!(read_sie() == 0);
            fn test() {
                enable_interrupts();
                loop {
                    println!("{:?}", "a calculatioN!");
                    kernel_cpu::wfi();
                }
            }
            disable_interrupts();
            let mut process = Process::new_supervisor(
                |mut process| {
                    process.name = Some(alloc::string::String::from("hello world"));
                    unsafe {
                        let table = kernel_paging::Table::<8>::from_satp(process.trap_frame.satp, phys_to_virt).as_ref().unwrap().clone();
                        let table = table.clone_with(phys_to_virt, virt_to_phys);
                        process.trap_frame.satp = table.to_satp_base_addr(virt_to_phys) | read_satp_flags();
                        process.page_table = Some(alloc::sync::Arc::new(table));
                    };
                },
                test,
                phys_to_virt,
                virt_to_phys,
            );
            //process.lock().trap_frame.satp = q;

            loop {
                // Make sure the process is ready for waking up
                wait_until_process_is_woken(&process).await;
                set_relative_timer(0x00010_0000);
                // This has the SIE bit disabled because
                // the interrupt will get triggered in the idle task.
                process.lock().trap_frame.sie = (!read_sip()) & 0x022;
                process.lock().switch_to_and_come_back();
                process = do_syscall_and_drop_if_exit(process, |p| {
                    handle_come_back_from_process(Some(p))
                })
                .unwrap();
            }
        })));
    } else {

        plic.set_enabled(10, true);
        HartLocals::current()
            .local_executor
            .as_ref()
            .unwrap()
            .spawn(Box::new(Box::pin(async {
                assert!(read_sie() == 0);
                fn test() {
                    enable_interrupts();
                    
                    use alloc::string::String;
                    
                    fn getchar() -> char {
                        let id = unsafe { do_supervisor_syscall_2(10, 10, 0) };
                        unsafe { do_supervisor_syscall_1(2, id.0) };
                        do_supervisor_syscall_0(3);
                        
                        (unsafe {
                            ((0x1000_0000 as *const u8).add(GAP.load(Ordering::Relaxed)))
                                .read_volatile()
                        }) as char
                    }
                    fn readline() -> String {
                    	let mut string = String::new();
                    	loop {
                            let c = getchar();
							let c = if c == '\r' { '\n' } else { c };
							if c == 0x7f as char {
								// 0x7f == delete
								// 0x08 == backspace (go back 1 column)
								if !string.is_empty() {
									print!("\x08 \x08");
									string.pop();
								}
							} else {
								string.push(c);
								print!("{}", c);
							}
							if c == '\n' {
							    return string;
							}
                    	}
                    }
                    
                    loop {
                        let r = readline();
                        println!("You typed: {:?}", r);
                    }
                    unreachable!();
                    loop {}
                }
                disable_interrupts();
                let mut process = Process::new_supervisor(
                    |mut process| {
                        process.name = Some(alloc::string::String::from("hello world"));
                        unsafe {
                            let table = kernel_paging::Table::<8>::from_satp(process.trap_frame.satp, phys_to_virt).as_ref().unwrap().clone();
                            let table = table.clone_with(phys_to_virt, virt_to_phys);
                            process.trap_frame.satp = table.to_satp_base_addr(virt_to_phys) | read_satp_flags();
                            println!("{:x}", process.trap_frame.satp);
                            process.page_table = Some(alloc::sync::Arc::new(table));
                        };
                    },
                    test,
                    phys_to_virt,
                    virt_to_phys,
                );
                //process.lock().trap_frame.satp = q;

                loop {
                    // Make sure the process is ready for waking up
                    wait_until_process_is_woken(&process).await;
                    set_relative_timer(0x0010_0000);
                    // This has the SIE bit disabled because
                    // the interrupt will get triggered in the idle task.
                    process.lock().trap_frame.sie = (!read_sip()) & 0x022;
                    process.lock().switch_to_and_come_back();
                    process = do_syscall_and_drop_if_exit(process, |p| {
                        handle_come_back_from_process(Some(p))
                    })
                    .unwrap();
                }
            })));
    }
    let handle = HartLocals::current()
        .local_executor
        .as_ref()
        .unwrap()
        .clone();
    handle.await;

    loop {}
}

pub unsafe fn paging_from_satp(satp: usize) -> Box<dyn Paging> {
    match PagingMode::from_satp(satp) {
        PagingMode::Bare => {
            panic!("{:?}", "Running without paging!");
        }
        PagingMode::Sv32 => Box::new(kernel_paging::Sv32::from_satp(
            satp,
            phys_to_virt,
            virt_to_phys,
        )),
        PagingMode::Sv39 => Box::new(kernel_paging::Sv39::from_satp(
            satp,
            phys_to_virt,
            virt_to_phys,
        )),
        PagingMode::Sv48 => Box::new(kernel_paging::Sv48::from_satp(
            satp,
            phys_to_virt,
            virt_to_phys,
        )),
    }
}

#[no_mangle]
fn test_fn() {}

#[no_mangle]
fn syscall_on_interrupt_disabled() {
    println!("{:?}", "interrupt disbled! cant syscall.");
    loop {}
}


pub use kernel_util::debug::get_uart;

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
