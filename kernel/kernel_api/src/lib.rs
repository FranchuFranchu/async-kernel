#![no_std]

pub mod s_mode;

extern crate alloc;

use alloc::boxed::Box;
use s_mode::*;

#[derive(Clone, Debug)]
pub struct KernelFuture {
    id: u64,
}

pub enum SyscallNumbers {
    Exit = 0x1,
    EnableFuture = 0x2,
    Sleep = 0x3,
    Interrupt = 0xa,
    MoveOutBuffer = 0x10,
    BorrowOutBuffer = 0x11,
    MutablyBorrowOutBuffer = 0x12,
    CopyOutBuffer = 0x13,
    ClaimBuffer = 0x20,
}

impl KernelFuture {
    pub fn wait_for_complete(self) {
        do_supervisor_syscall_1(SyscallNumbers::EnableFuture as usize, self.id as usize);
        do_supervisor_syscall_0(SyscallNumbers::Sleep as usize);
    }
}

pub struct BufferQueue {
    id: u64
}

impl BufferQueue {
    fn new(id: u64) -> BufferQueue {
        Self {
            id
        }
    }
    pub fn move_out_buffer(&self, buffer: Box<[u8]>) -> KernelFuture{
        assert!(buffer.as_ptr() as usize & 0xFFF == 0);
        assert!(buffer.len() & 0xFFF == 0);
        let ret = do_supervisor_syscall_3(SyscallNumbers::MoveOutBuffer as usize, buffer.as_ptr() as usize, buffer.len(), self.id as usize);
        KernelFuture { id: ret.0 as u64 }
    }
    pub fn claim_buffer(&self)  {
        
    }
}
