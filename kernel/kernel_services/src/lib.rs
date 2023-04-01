#![no_std]
#![feature(ptr_as_uninit)]

use core::{mem::{MaybeUninit, size_of_val}, mem::size_of, sync::atomic::AtomicUsize};

use alloc::{collections::BTreeSet, boxed::Box};
use kernel_api::BufferQueue;

extern crate alloc;

trait AsBuffer {
    fn serialize_raw(&self) -> &[MaybeUninit<u8>];
    fn serialize_raw_mut(&mut self) -> &mut [MaybeUninit<u8>];
    fn deserialize_raw(buf: &[MaybeUninit<u8>]) -> &MaybeUninit<Self> where Self: Sized;
    fn deserialize_raw_mut(buf: &mut [MaybeUninit<u8>]) -> &mut MaybeUninit<Self> where Self: Sized;
}

impl<T> AsBuffer for T where T: Sized {
    fn serialize_raw(&self) -> &[MaybeUninit<u8>] {
        unsafe {
            core::slice::from_raw_parts(self as *const Self as *const MaybeUninit<u8>, size_of_val(self))
        }
    }
    fn serialize_raw_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        unsafe {
            core::slice::from_raw_parts_mut(self as *const Self as *const MaybeUninit<u8> as *mut _, size_of_val(self))
        }
    }
    fn deserialize_raw(buf: &[MaybeUninit<u8>]) -> &MaybeUninit<Self> {
        assert!(buf.len() >= size_of::<Self>());
        unsafe { 
            (buf.as_ptr() as *const MaybeUninit<u8> as *const Self).as_uninit_ref().unwrap()
        }
    }
    fn deserialize_raw_mut(buf: &mut [MaybeUninit<u8>]) -> &mut MaybeUninit<Self> {
        assert!(buf.len() >= size_of::<Self>());
        unsafe { 
            (buf.as_ptr() as *mut MaybeUninit<u8> as *mut Self).as_uninit_mut().unwrap()
        }
    }
}

pub enum CommandHeader {
    Empty,
    GetName(u64, [u8; 124]),
    SetName(u64, [u8; 124]),
}
/*
pub fn queue_name_service() {
    let source_queue = BufferQueue::new(0);
    let new_queue_buffer = 0u64;
    source_queue.copy_claim_buffer(destination);
    
    let mut command_buffer = CommandHeader::Empty;
    queue.copy_claim_buffer(command_buffer.serialize_raw_mut());
    command_buffer
}*/


#[derive(Default)]
#[repr(align(4096))]
pub enum ReserveQueueRequest {
    #[default]
    Def,
    Free(usize),
    Allocate(AtomicUsize),
}

pub fn reserve_queue_service() {
    let mut reserved = BTreeSet::new();
    
    let source_queue = BufferQueue::new(0);
    let mut command = Box::new(ReserveQueueRequest::default());
    source_queue.copy_claim_buffer(command.serialize_raw_mut());
    use ReserveQueueRequest::*;
    match *command {
        Def => {},
        Free(n) => {
            reserved.remove(&n);
        }
        Allocate(buf) => {
            let n = reserved.iter().max().unwrap_or(&0) + 1;
            reserved.insert(n);
            buf.store(n, core::sync::atomic::Ordering::Release);
        }
    }
}