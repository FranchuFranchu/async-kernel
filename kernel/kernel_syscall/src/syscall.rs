use alloc::boxed::Box;

use kernel_cpu::Registers;
use kernel_process::{Process, ProcessContainer, ProcessState};
use num_enum::*;

#[derive(FromPrimitive, IntoPrimitive)]
#[repr(usize)]
pub enum SyscallNumbers {
    Exit = 1,
    #[default]
    Unknown,
}

pub fn get_syscall_args(process: &mut Process) -> &mut [usize] {
    &mut process.trap_frame.general_registers[Registers::A0.idx()..=Registers::A7.idx()]
}

pub fn do_syscall_and_drop_if_exit(
    process: ProcessContainer,
    syscall_fn: impl FnOnce(&mut Process),
) -> Option<ProcessContainer> {
    let mut lock = process.lock();
    syscall_fn(&mut *lock);

    let lock_two = lock.wake_on_paused.lock();
    let state = lock_two.state.clone();
    drop(lock_two);
    match state {
        ProcessState::Exited => None,
        _ => {
            drop(lock);
            Some(process)
        }
    }
}
