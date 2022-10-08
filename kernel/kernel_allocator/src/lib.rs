#![no_std]
#![feature(int_roundings)]

use core::alloc::GlobalAlloc;

use slab_allocator_rs::LockedHeap;

extern crate kernel_util;

#[global_allocator]
static ALLOCATOR: ProxyAllocator = ProxyAllocator(LockedHeap::empty());

struct ProxyAllocator(LockedHeap);

unsafe impl GlobalAlloc for ProxyAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let sie_guard = kernel_cpu::sie_guard();
        self.0.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let sie_guard = kernel_cpu::sie_guard();
        self.0.dealloc(ptr, layout)
    }
}

pub fn init_from_pointers(start: *const (), end: *const ()) {
    // Initialize memory allocation
    let heap_end = end as usize;
    let mut heap_start = start as usize;

    heap_start = heap_start.div_ceil(slab_allocator_rs::MIN_HEAP_SIZE);
    heap_start *= slab_allocator_rs::MIN_HEAP_SIZE;

    let mut heap_size: usize = heap_end - heap_start;

    // Align the size to min heap size boundaries
    heap_size /= slab_allocator_rs::MIN_HEAP_SIZE;
    heap_size *= slab_allocator_rs::MIN_HEAP_SIZE;

    init_heap(heap_start, heap_size);
}

pub fn init_heap(heap_start: usize, heap_size: usize) {
    unsafe {
        ALLOCATOR.0.init(heap_start, heap_size);
    }
}
