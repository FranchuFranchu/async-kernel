#![no_std]
//! Abstractions over the RISC-V Supervisor Binary Interface to communicate with M-mode code
// See https://github.com/riscv/riscv-sbi-doc/blob/master/riscv-sbi.adoc
use core::arch::asm;

#[repr(isize)]
#[derive(Debug)]
pub enum SBIError {
    Success,
    Failed,
    NotSupported,
    InvalidParam,
    Denied,
    InvalidAddress,
    AlreadyAvailable,
    AlreadyStarted,
    AlreadyStopped,
    Unknown,
}

// TODO Add rust macro for this
impl SBIError {
    fn from_isize(v: isize) -> Self {
        use SBIError::*;
        match v {
            0 => Success,
            -1 => Failed,
            -2 => NotSupported,
            -3 => InvalidParam,
            -4 => Denied,
            -5 => InvalidAddress,
            -6 => AlreadyAvailable,
            -7 => AlreadyStarted,
            -8 => AlreadyStopped,
            _ => Unknown,
        }
    }
}

pub unsafe fn call_sbi_0(extension_id: usize, function_id: usize) -> Result<usize, SBIError> {
    let error_code: usize;
    let return_value: usize;
    asm!(r"
        mv a7, {2}
        mv a6, {3}
        ecall
        mv {0}, a0
        mv {1}, a1
    ", out(reg) error_code, out(reg) return_value, in(reg) extension_id, in(reg) function_id);

    if error_code == 0 {
        Ok(return_value)
    } else {
        Err(SBIError::from_isize(core::mem::transmute(return_value)))
    }
}

pub unsafe fn call_sbi_1(
    extension_id: usize,
    function_id: usize,
    a0: usize,
) -> Result<usize, SBIError> {
    let error_code: usize;
    let return_value: usize;
    asm!(r"
        mv a7, {2}
        mv a6, {3}
        mv a0, {4}
        ecall
        mv {0}, a0
        mv {1}, a1
    ", out(reg) error_code, out(reg) return_value, in(reg) extension_id, in(reg) function_id, in(reg) a0);

    if error_code == 0 {
        Ok(return_value)
    } else {
        Err(SBIError::from_isize(core::mem::transmute(return_value)))
    }
}

pub unsafe fn call_sbi_2(
    extension_id: usize,
    function_id: usize,
    a0: usize,
    a1: usize,
) -> Result<usize, SBIError> {
    let error_code: usize;
    let return_value: usize;
    asm!(r"
        mv a7, {2}
        mv a6, {3}
        mv a0, {4}
        mv a1, {5}
        ecall
        mv {0}, a0
        mv {1}, a1
    ", out(reg) error_code, out(reg) return_value, in(reg) extension_id, in(reg) function_id, in(reg) a0, in(reg) a1);

    if error_code == 0 {
        Ok(return_value)
    } else {
        Err(SBIError::from_isize(core::mem::transmute(return_value)))
    }
}

pub unsafe fn call_sbi_3(
    extension_id: usize,
    function_id: usize,
    a0: usize,
    a1: usize,
    a2: usize,
) -> Result<usize, SBIError> {
    let error_code: usize;
    let return_value: usize;
    asm!(r"
        mv a7, {2}
        mv a6, {3}
        mv a0, {4}
        mv a1, {5}
        mv a2, {6}
        ecall
        mv {0}, a0
        mv {1}, a1
    ", out(reg) error_code, out(reg) return_value, in(reg) extension_id, in(reg) function_id, in(reg) a0, in(reg) a1, in(reg) a2);

    if error_code == 0 {
        Ok(return_value)
    } else {
        Err(SBIError::from_isize(core::mem::transmute(return_value)))
    }
}

pub fn set_absolute_timer(time: u64) -> Result<(), SBIError> {
    // SAFETY: Assuming the SBI implementation is correct, setting a timer shouldn't cause anything bad in memory
    // Note that this SBI call's return value is meaningless, so we erase it
    // TODO: Use RV32 SBI for u64's here
    unsafe { call_sbi_1(0x54494D45, 0, time as usize).map(|_| {}) }
}

pub fn shutdown(reason: usize) {
    // SAFETY: Shutting down is safe, because the whole machine state gets erased. But destructors don't get called
    unsafe {
        // See https://github.com/riscv/riscv-sbi-doc/blob/master/riscv-sbi.adoc#system-reset-extension-eid-0x53525354-srst
        call_sbi_2(0x53525354, 0, 0, reason);
    }
}

/// Safety: Only if start_addr is an address capable of bootstrapping himself
pub unsafe fn start_hart(hartid: usize, start_addr: usize, opaque: usize) -> Result<(), SBIError> {
    call_sbi_3(0x48534D, 0, hartid, start_addr, opaque).map(|_| {})
}

pub fn hart_get_status(hartid: usize) -> Result<usize, SBIError> {
    unsafe { call_sbi_1(0x48534D, 2, hartid) }
}
