//! Functions that run specific RISC-V instructions
#![no_std]

use core::arch::asm;

// this enum is from osblog
#[derive(Copy, Clone)]
#[repr(usize)]
pub enum Registers {
    Zero = 0,
    Ra,
    Sp,
    Gp,
    Tp,
    T0,
    T1,
    T2,
    S0,
    S1,
    A0, /* 10 */
    A1,
    A2,
    A3,
    A4,
    A5,
    A6,
    A7,
    S2,
    S3,
    S4, /* 20 */
    S5,
    S6,
    S7,
    S8,
    S9,
    S10,
    S11,
    T3,
    T4,
    T5, /* 30 */
    T6,
}

impl Registers {
    pub const fn idx(&self) -> usize {
        *self as usize
    }
}

/// # Safety
/// This can cause hangups and other things that aren't very good
#[inline(always)]
pub unsafe fn write_sie(value: usize) {
    asm!("csrw sie, {0}" , in(reg) ( value) )
}

/// # Safety
/// When setting interrupts, the proper context needs to be created for the trap
/// handler
#[inline(always)]
pub unsafe fn write_sip(value: usize) {
    asm!("csrw sip, {0}" , in(reg) ( value) )
}

/// # Safety
/// Must be s_trap
#[inline(always)]
pub unsafe fn write_stvec(value: usize) {
    asm!("csrw stvec, {0}" , in(reg) ( value) )
}

/// # Safety
/// Must uphold SATP assumptions in the rest of the kernel. Mainly, that it's a
/// valid page table
#[inline(always)]
pub unsafe fn write_satp(value: usize) {
    asm!("
        csrw satp, {0}
        sfence.vma
        ", in(reg) value)
}

/// # Safety
/// Too many constraints to document. Shouldn't be changed very frecuently.
#[inline(always)]
pub unsafe fn write_sstatus(value: usize) {
    asm!("csrw sstatus, {0}" , in(reg) ( value) )
}

/// This is unsafe because other parts of the kernel rely on sscratch being a
/// valid pointer # Safety
/// Must be a valid trap frame and must make sense with what the hart is
/// executing
#[inline(always)]
pub unsafe fn write_sscratch(value: usize) {
    asm!("csrw sscratch, {0}" , in(reg) ( value) )
}

#[inline]
pub fn read_sscratch() -> *mut kernel_trap_frame::TrapFrame {
    let value: usize;
    unsafe { asm!("csrr {0}, sscratch", out(reg)(value),) };
    value as _
}

#[inline(always)]
pub fn read_sp() -> usize {
    let value: usize;
    unsafe { asm!("mv {0}, sp", out(reg)(value),) };
    value
}

#[inline(always)]
pub fn read_sip() -> usize {
    let value: usize;
    unsafe { asm!("csrr {0}, sip", out(reg)(value),) };
    value
}

#[inline(always)]
pub fn read_satp() -> usize {
    let value: usize;
    unsafe { asm!("csrr {0}, satp", out(reg)(value),) };
    value
}

#[inline(always)]
pub fn read_sie() -> usize {
    let value: usize;
    unsafe { asm!("csrr {0}, sie", out(reg)(value),) };
    value
}

#[inline(always)]
pub fn read_sstatus() -> usize {
    let value: usize;
    unsafe { asm!("csrr {0}, sstatus", out(reg)(value),) };
    value
}

#[inline(always)]
pub fn read_stval() -> usize {
    let value: usize;
    unsafe { asm!("csrr {0}, stval", out(reg)(value),) };
    value
}

#[inline(always)]
pub fn read_scause() -> usize {
    let value: usize;
    unsafe { asm!("csrr {0}, scause", out(reg)(value),) };
    value
}
#[inline(always)]
pub fn read_time() -> u64 {
    #[cfg(target_arch = "riscv32")]
    {
        let timeh: u32;
        let timel: u32;
        unsafe { asm!("csrr {0}, timeh", out(reg)(timeh),) };
        unsafe { asm!("csrr {0}, time", out(reg)(timel),) };
        timeh << 32 | timel
    }
    #[cfg(target_arch = "riscv64")]
    {
        let timel: u64;
        unsafe { asm!("csrr {0}, time", out(reg)(timel),) };
        timel
    }
}

#[inline(always)]
pub fn read_cycle() -> usize {
    let value: usize;
    unsafe { asm!("csrr {0}, cycle", out(reg)(value),) };
    value
}

#[inline(always)]
pub fn read_instret() -> usize {
    let value: usize;
    unsafe { asm!("csrr {0}, instret", out(reg)(value),) };
    value
}
/// Gets hartid from sscratch
/// This assumes that sscratch holds a valid value
pub fn load_hartid() -> usize {
    unsafe { (*read_sscratch()).hartid }
}

use core::sync::atomic::AtomicUsize;

use csr::XCAUSE_DESCRIPTION;
pub static BOOT_HART: AtomicUsize = AtomicUsize::new(0);

#[inline(always)]
pub fn wfi() {
    // SAFETY:
    // wfi never changes any register state and is always safe
    // it's essentially a processor hint and can act as a NOP
    unsafe {
        asm!("wfi");
    }
}

#[inline(always)]
pub fn fence_vma() {
    unsafe {
        asm!(
            "
        sfence.vma zero, zero
        fence rw, rw
    "
        )
    };
}

#[inline]
pub fn in_interrupt_context() -> bool {
    // TODO make this sound (aliasing rules?)
    (read_sscratch() as usize == 0)
        || unsafe { read_sscratch().as_ref().unwrap().is_interrupt_context() }
}

pub struct SieGuard(usize);

impl SieGuard {
    pub fn new() -> Self {
        let old = read_sie();
        unsafe {
            write_sie(0);
        }
        Self(old)
    }
}

impl Drop for SieGuard {
    fn drop(&mut self) {
        unsafe {
            write_sie(self.0);
        }
    }
}

pub fn sie_guard() -> SieGuard {
    SieGuard::new()
}

#[inline]
pub fn set_interrupt_context() {
    unsafe { (*read_sscratch()).flags |= 1 }
}

#[inline]
pub fn clear_interrupt_context() {
    unsafe { (*read_sscratch()).flags &= !1 }
}

#[inline]
pub fn read_satp_flags() -> usize {
    #[cfg(target_arch = "riscv32")]
    {
        return ((read_satp()) & (0x3 << 30));
    }
    #[cfg(target_arch = "riscv64")]
    {
        return ((read_satp()) & (0xF << 62));
    }
    unreachable!();
}

#[inline]
pub fn read_satp_table_addr() -> usize {
    read_satp() << 12
}

#[inline]
pub fn is_paging_enabled() -> bool {
    read_satp_flags() != 0
}

pub fn get_xcause_explanation(cause: usize) -> &'static str {
    XCAUSE_DESCRIPTION[cause]
}

// This module describes CSR bits and layouts
pub mod csr {
    // First are the xip and xep CSRs
    // In the first characture, U means user, S means supervisor, and M means
    // machine In the second one, S means software, T means timer, and E means
    // external

    // For the xip CSRS (interrupt pending)
    // Software
    pub const USIP: usize = 1 << 0;
    pub const SSIP: usize = 1 << 1;
    pub const MSIP: usize = 1 << 3;

    // Timer
    pub const UTIP: usize = 1 << 4;
    pub const STIP: usize = 1 << 5;
    pub const MTIP: usize = 1 << 7;

    // External (PLIC)
    pub const UEIP: usize = 1 << 8;
    pub const SEIP: usize = 1 << 9;
    pub const MEIP: usize = 1 << 11;

    // For the xie CSRS (interrupt enable)
    // Software
    pub const USIE: usize = 1 << 0;
    pub const SSIE: usize = 1 << 1;
    pub const MSIE: usize = 1 << 3;

    // Timer
    pub const UTIE: usize = 1 << 4;
    pub const STIE: usize = 1 << 5;
    pub const MTIE: usize = 1 << 7;

    // External
    pub const UEIE: usize = 1 << 8;
    pub const SEIE: usize = 1 << 9;
    pub const MEIE: usize = 1 << 11;

    /// XSTATUS
    pub mod status {
        pub const SIE: usize = 1 << 1;
        pub const MIE: usize = 1 << 3;
        pub const SPIE: usize = 1 << 5;
        pub const MPIE: usize = 1 << 7;
        pub const SPP: usize = 1 << 8;
    }
    /// XCAUSE
    pub mod cause {
        pub const SUPERVISOR_SOFTWARE: usize = 1;
        pub const VIRTUAL_SUPERVISOR_SOFTWARE: usize = 2;
        pub const MACHINE_SOFTWARE: usize = 3;
        pub const SUPERVISOR_TIMER: usize = 5;
        pub const VIRTUAL_SUPERVISOR_TIMER: usize = 6;
        pub const MACHINE_TIMER: usize = 7;
        pub const SUPERVISOR_EXTERNAL: usize = 9;
        pub const VIRTUAL_SUPERVISOR_EXTERNAL: usize = 10;
        pub const MACHINE_EXTERNAL: usize = 11;
        pub const SUPERVISOR_GUEST_EXTERNAL: usize = 12;
    }

    // SATP flags
    pub const SATP_BARE: usize = 0;
    pub const SATP_SV32: usize = 1 << 30;
    #[cfg(target_arch = "riscv64")]
    pub const SATP_SV39: usize = 8 << 60;
    #[cfg(target_arch = "riscv64")]
    pub const SATP_SV48: usize = 9 << 60;

    pub enum PagingMode {
        Bare,
        Sv32,
        Sv39,
        Sv48,
    }

    impl PagingMode {
        pub fn from_satp(satp: usize) -> PagingMode {
            let mode_bitfield = if cfg!(target_arch = "riscv32") {
                (satp >> 30) & 0b11
            } else if cfg!(target_arch = "riscv64") {
                (satp >> 60) & 0b1111
            } else {
                unreachable!("Not RV32 or RV64!");
            };
            match mode_bitfield {
                0 => Self::Bare,
                1 => Self::Sv32,
                8 => Self::Sv39,
                9 => Self::Sv48,
                _ => todo!(),
            }
        }
    }

    pub const XCAUSE_DESCRIPTION: [&str; 16] = [
        "Instruction address misaligned",
        "Instruction access fault",
        "Illegal instruction",
        "Breakpoint",
        "Load address misaligned",
        "Load access fault",
        "Store/AMO address misaligned",
        "Store/AMO access fault",
        "Enviornment call from U-mode",
        "Enviornment call from S-mode",
        "<Reserved>",
        "Enviornment call from M-mode",
        "Instruction page fault",
        "Load page fault",
        "<Reserved>",
        "Store/AMO page fault",
    ];
}
