//! Maps the kernel to the higher half, then jumps to it.
#![no_std]
#![no_main]
#![feature(bench_black_box, default_alloc_error_handler)]
use core::ffi::c_void;
core::arch::global_asm!(include_str!("../boot.S"));

// Linker symbols
extern "C" {
    static _heap_start: c_void;
    static _heap_end: c_void;

    static _stack_start: c_void;
    static _stack_end: c_void;

    fn s_trap_vector();
    fn new_hart();
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

extern crate kernel_allocator;


#[no_mangle]
pub unsafe extern "C" fn pre_main(hartid: usize, opaque: usize) {
    core::hint::black_box(pre_main);
    
    //unsafe { kernel_allocator::init_from_pointers(&_heap_start as *const _ as *const _, &_heap_end as *const _ as *const _) };
    let sv_bits = 39;
    let gap = (0usize.wrapping_sub(1 << 38));
    
    let mborrow = unsafe { &mut TABLE };
    let mut root_table = kernel_paging::gpaging::Sv39 { table: mborrow };
    
    unsafe { root_table.map(0, 0, 1 << 38, 0xf) };
    unsafe { root_table.map(0, 1 << 38, 1 << 38, 0xf) };
    
    let addr = mborrow as *mut _ as usize;
    assert!(addr & 4095 == 0);
    let mut satp = (addr) >> 12;
	unsafe { (0x1000_0000 as *mut u8).write_volatile(65) };
    println!("{:x}", pre_main as usize);
    satp |= 8 << 60;
    unsafe {
        core::arch::asm!("
            csrw satp, {0}
            sfence.vma
            ", in(reg) satp)
    }
    
    let mborrow = unsafe { &mut TABLE };
    let mut root_table = kernel_paging::gpaging::Sv39 { table: mborrow };
    
    let start: usize = unsafe { &_heap_start as *const _ as usize };;
    let end: usize = 0x8200_0000;
    
    kernel_allocator::init_from_pointers(start as *const _, end as *const _);
    println!("{:x}", ((ALIGNED_BYTES.len()) / 4096 + 1) * 4096);
    root_table.map(
        ALIGNED_BYTES.as_ptr() as usize, 
        0x7f80000000, 
        ((ALIGNED_BYTES.len()) / 4096 + 1) * 4096, 
        0xf);
    let main = core::mem::transmute::<usize, fn(usize, usize)>(0xffffffff80000000);
    main(hartid, opaque);
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
static ALIGNED: &'static AlignedTo<Align4096, [u8]> = &AlignedTo {
    _align: [],
    bytes: *include_bytes!("../../../kernel_payload.bin"),
};

static ALIGNED_BYTES: &'static [u8] = &ALIGNED.bytes;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{:?}", "Bootloader Panic");
    println!("{:?}", info);
    loop {}
}
