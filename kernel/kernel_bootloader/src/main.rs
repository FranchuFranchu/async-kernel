//! Maps the kernel to the higher half, then jumps to it.
#![no_std]
#![no_main]
#![feature(bench_black_box, default_alloc_error_handler, naked_functions)]
use core::{ffi::c_void, sync::atomic::AtomicBool, ops::{BitAnd, Add}};

use kernel_cpu::{write_stvec, write_satp, fence_vma};
core::arch::global_asm!(include_str!("../boot.S"));

// Linker symbols
extern "C" {
    static _heap_start: c_void;
    static _heap_end: c_void;

    static _stack_start: c_void;
    static _stack_end: c_void;

    fn new_hart();
    fn hart_entry_point();
}

#[naked]
unsafe extern "C" fn s_trap_vector() {
    core::arch::asm!("
        nop
        nop
        la sp, _stack_start
        j trap_handler
    ", options(noreturn) );
}

#[no_mangle]
unsafe fn trap_handler() {
    write_satp(0);
    fence_vma();
    println!("{:?}", "Kernel trapped before setting up trap handler!");
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

extern crate kernel_allocator;

pub fn phys_to_virt(phys_addr: usize) -> usize {
    phys_addr
}

pub fn virt_to_phys(virt_addr: usize) -> usize {
    virt_addr
}

#[no_mangle]
pub extern "C" fn pre_main(hartid: usize, opaque: usize) {
    static ALREADY_BOOTED: AtomicBool = AtomicBool::new(false);
    core::hint::black_box(pre_main);

    if ALREADY_BOOTED.swap(true, core::sync::atomic::Ordering::Relaxed) {
        println!("{:?}", "Already booted!");
        loop {}
    }
    
    unsafe { write_stvec((s_trap_vector as usize).bitand(!3).add(4)) };

    // TODO: Determine the paging scheme that will be used
    let sv_bits = 39;
    let gap = 0usize.wrapping_sub(1 << 38);

    // Create page tables that will be used for both us and the kernel payload
    // 0..gap will be mapped 1:1 physical memory
    // gap..usize::MAX will be mapped 1:1 to physical memory
    let mborrow = unsafe { &mut TABLE };
    let mut root_table = kernel_paging::Sv39 {
        table: mborrow,
        phys_to_virt,
        virt_to_phys,
    };

    unsafe { root_table.map(0, 0, 1 << 38, 0xf) };
    unsafe { root_table.map(0, 1 << 38, 1 << 38, 0xf) };

    let addr = mborrow as *mut _ as usize;
    assert!(addr & 4095 == 0);
    let mut satp = (addr) >> 12;
    satp |= 8 << 60;

    unsafe {
        core::arch::asm!("
            csrw satp, {0}
            sfence.vma
            ", in(reg) satp)
    }

    let mborrow = unsafe { &mut TABLE };
    let mut root_table = kernel_paging::Sv39 {
        table: mborrow,
        phys_to_virt,
        virt_to_phys,
    };

    let start: usize = unsafe { &_heap_start as *const _ as usize };
    let end: usize = start + 0x10000;

    kernel_allocator::init_from_pointers(start as *const _, end as *const _);
    let padded_len = ((ALIGNED_BYTES.len()) / 4096 + 1) * 4096;

    // Change the page table mapping
    // Now, in addition to the above mappings:
    // usize::MAX-0x80000000..usize::MAX-0x80000000+payload.len() will be
    // mapped to the kernel payload.
    // The kernel payload has been linked with usize::MAX-0x80000000 as its
    // base virtual address,
    root_table.map(
        ALIGNED_BYTES.as_ptr() as usize,
        0x7f80000000,
        ((ALIGNED_BYTES.len()) / 4096 + 1) * 4096,
        0xf,
    );
    

    unsafe {
        let main = core::mem::transmute::<
            usize,
            extern "C" fn(usize, usize, usize, usize, usize, usize),
        >(0xffffffff80000000);
        main(
            hartid,
            opaque,
            sv_bits,
            padded_len,
            &_stack_start as *const _ as usize + gap,
            hart_entry_point as usize,
        );
    }
}

#[no_mangle]
pub extern "C" fn hart_entry(hartid: usize) {
    unsafe {
        let main = core::mem::transmute::<
            usize,
            extern "C" fn(usize, usize, usize, usize, usize, usize),
        >(0xffffffff80000000);
        main(
            hartid,
            0,
            39,
            0,
            kernel_cpu::read_sp() + 0xffffffc000000000,
            hart_entry_point as usize,
        );
    }
}

static mut TABLE: kernel_paging::Table<8> = kernel_paging::Table::zeroed();

#[repr(C, align(4096))]
pub struct Align4096;

// This struct is generic in Bytes to admit unsizing coercions.
#[repr(C)] // guarantee 'bytes' comes after '_align'
struct AlignedTo<Align, Bytes: ?Sized> {
    _align: [Align; 0],
    bytes: Bytes,
}

// dummy static used to create aligned data
static ALIGNED: &AlignedTo<Align4096, [u8]> = &AlignedTo {
    _align: [],
    bytes: *include_bytes!("../../../kernel_payload.bin"),
};

static ALIGNED_BYTES: &[u8] = &ALIGNED.bytes;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{:?}", "Bootloader Panic");
    println!("{:?}", info);
    loop {}
}
