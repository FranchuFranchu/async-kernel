#![no_std]

use slab_allocator_rs::LockedHeap;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub fn init_from_pointers(start: *const (), end: *const ()) {
    // Initialize memory allocation
    let heap_end = unsafe { end as usize };
    let heap_start = unsafe { start as usize };
    let mut heap_size: usize = heap_end - heap_start;

    // Align the size to min heap size boundaries
    heap_size /= slab_allocator_rs::MIN_HEAP_SIZE;
    heap_size *= slab_allocator_rs::MIN_HEAP_SIZE;

    init_heap(heap_start, heap_size)
}


pub fn init_heap(heap_start: usize, heap_size: usize) {
    unsafe {
        ALLOCATOR.init(heap_start, heap_size);
    }
}
