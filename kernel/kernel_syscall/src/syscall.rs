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

/// `process` is a process that just executed a system call
/// either by setting SSIP or calling ECALL
pub fn do_syscall(process: &mut Process) {
    let mut arguments =
        &mut process.trap_frame.general_registers[Registers::A0.idx()..=Registers::A7.idx()];

    let syscall_number_value = arguments.last().unwrap();
    let syscall_number = SyscallNumbers::from(*syscall_number_value);

    match syscall_number {
        Exit => {
            process.state = ProcessState::Exited;
        }
        Unknown => {
            println!("Unknown syscall {}", syscall_number_value);
        }
    }
}

pub fn do_syscall_and_drop_if_exit(process: ProcessContainer) -> Option<ProcessContainer> {
    let mut lock = process.lock();
    do_syscall(&mut *lock);

    match lock.state {
        ProcessState::Exited => None,
        _ => {
            drop(lock);
            Some(process)
        }
    }
}
