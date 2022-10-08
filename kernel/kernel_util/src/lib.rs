#![no_std]

use core::{
    alloc::Layout,
    mem::{size_of, MaybeUninit},
};

pub mod debug;
pub mod mem;
pub mod maybe_waker;

extern crate alloc;

#[macro_export]
macro_rules! useful_asm_fragment {
	(boot, $main:expr) => (core::arch::asm!("
		mv sp, a4
		la t0, {0}
		jalr t0
	",
	sym $main,
	options(noreturn)
	));
}

#[macro_export]
macro_rules! get_symbol_addr {
    ($symbol:ident) => {
        (&$symbol as *const _) as usize
    };
}

pub fn boxed_slice_with_alignment<T: Clone>(
    size: usize,
    align: usize,
    initialize: &T,
) -> alloc::boxed::Box<[T]> {
    unsafe {
        let ptr: *mut MaybeUninit<T> =
            alloc::alloc::alloc(Layout::from_size_align(size * size_of::<T>(), align).unwrap())
                as *mut MaybeUninit<T>;
        for i in 0..size {
            *ptr.add(i) = MaybeUninit::new(initialize.clone())
        }
        alloc::boxed::Box::from_raw(core::slice::from_raw_parts_mut(ptr as *mut T, size))
    }
}
pub fn boxed_slice_with_alignment_uninit<T>(
    size: usize,
    align: usize,
) -> alloc::boxed::Box<[MaybeUninit<T>]> {
    unsafe {
        let ptr: *mut MaybeUninit<T> =
            alloc::alloc::alloc(Layout::from_size_align(size * size_of::<T>(), align).unwrap())
                as *mut MaybeUninit<T>;
        alloc::boxed::Box::from_raw(core::slice::from_raw_parts_mut(ptr, size))
    }
}
