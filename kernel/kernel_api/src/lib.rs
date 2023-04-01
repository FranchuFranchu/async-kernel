#![no_std]

pub mod s_mode;

extern crate alloc;

use core::mem::MaybeUninit;

use alloc::boxed::Box;
use s_mode::*;

#[derive(Clone, Debug)]
pub struct KernelFuture {
    id: u64,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
#[repr(usize)]
pub enum SyscallNumbers {
    Exit = 1,
    EnableFuture = 2,
    Sleep = 3,
    PollFuture = 4,
    WaitForInterrupt = 10,
    MoveBufferOut = 0x10,
    BorrowBufferOut = 0x11,
    BorrowMutBufferOut = 0x12,
    CopyBufferOut = 0x13,
    MapBufferIn = 0x20,
    CopyBufferIn = 0x21,
}

impl KernelFuture {
    pub fn poll(&self) -> Result<core::task::Poll<()>, ()> {
        let result = do_supervisor_syscall_1(SyscallNumbers::PollFuture as usize, self.id as usize);
        match result.0 {
            1 => Ok(core::task::Poll::Pending),
            2 => Ok(core::task::Poll::Ready(())),
            _ => Err(()),
        }
    }
    pub fn wait_for_complete(&self) {
        while self.poll() == Ok(core::task::Poll::Pending) {
            do_supervisor_syscall_1(SyscallNumbers::EnableFuture as usize, self.id as usize);
            do_supervisor_syscall_0(SyscallNumbers::Sleep as usize);
        }
    }
}

pub struct BufferQueue {
    id: u64
}

impl BufferQueue {
    pub fn new(id: u64) -> BufferQueue {
        Self {
            id
        }
    }
    pub fn move_out_buffer(&self, buffer: Box<[MaybeUninit<u8>]>) -> KernelFuture{
        assert!(buffer.as_ptr() as usize & 0xFFF == 0);
        assert!(buffer.len() & 0xFFF == 0);
        let ret = do_supervisor_syscall_3(SyscallNumbers::MoveBufferOut as usize, buffer.as_ptr() as usize, buffer.len(), self.id as usize);
        KernelFuture { id: ret.0 as u64 }
    }
    pub fn share_claim_buffer(&self, destination: &mut [MaybeUninit<u8>]) -> Result<(usize, usize), KernelFuture> {
        let ret = do_supervisor_syscall_3(SyscallNumbers::MapBufferIn as usize, destination.as_ptr() as usize, destination.len(), self.id as usize);
        let status = ret.0;
        let real_size = ret.1;
        let remaining_buffers = ret.2;
        let future_id = ret.3;
        if status == 0 {
            return Err(KernelFuture { id: future_id as u64 })
        } else {
            return Ok((real_size, remaining_buffers))
        }
    }
    pub fn copy_claim_buffer(&self, destination: &mut [MaybeUninit<u8>]) -> Result<(usize, usize), KernelFuture> {
        let ret = do_supervisor_syscall_3(SyscallNumbers::CopyBufferIn as usize, destination.as_ptr() as usize, destination.len(), self.id as usize);
        let status = ret.0;
        let real_size = ret.1;
        let remaining_buffers = ret.2;
        let future_id = ret.3;
        if status == 0 {
            return Err(KernelFuture { id: future_id as u64 })
        } else {
            return Ok((real_size, remaining_buffers))
        }
    }
}
