pub fn do_supervisor_syscall_0(number: usize) -> (usize, usize, usize, usize, usize, usize, usize) { do_supervisor_syscall_7(number, 0, 0, 0, 0, 0, 0, 0) }
pub fn do_supervisor_syscall_1(number: usize, a0: usize) -> (usize, usize, usize, usize, usize, usize, usize) { do_supervisor_syscall_7(number, a0, 0, 0, 0, 0, 0, 0) }
pub fn do_supervisor_syscall_2(number: usize, a0: usize, a1: usize) -> (usize, usize, usize, usize, usize, usize, usize) { do_supervisor_syscall_7(number, a0, a1, 0, 0, 0, 0, 0) }
pub fn do_supervisor_syscall_3(number: usize, a0: usize, a1: usize, a2: usize) -> (usize, usize, usize, usize, usize, usize, usize) { do_supervisor_syscall_7(number, a0, a1, a2, 0, 0, 0, 0) }
pub fn do_supervisor_syscall_4(number: usize, a0: usize, a1: usize, a2: usize, a3: usize) -> (usize, usize, usize, usize, usize, usize, usize) { do_supervisor_syscall_7(number, a0, a1, a2, a3, 0, 0, 0) }
pub fn do_supervisor_syscall_5(number: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize) -> (usize, usize, usize, usize, usize, usize, usize) { do_supervisor_syscall_7(number, a0, a1, a2, a3, a4, 0, 0) }
pub fn do_supervisor_syscall_6(number: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize) -> (usize, usize, usize, usize, usize, usize, usize) { do_supervisor_syscall_7(number, a0, a1, a2, a3, a4, a5, 0) }



pub fn do_supervisor_syscall_7(
    mut number: usize,
    mut a0: usize,
    mut a1: usize,
    mut a2: usize,
    mut a3: usize,
    mut a4: usize,
    mut a5: usize,
    mut a6: usize,
    ) -> (usize, usize, usize, usize, usize, usize, usize) {
    unsafe { 
        core::arch::asm!("
    # csrr t0, sie
    # beqz t0, .error_syscall_interrupt_disabled
    
    
    # Set the supervisor software interrupt pending bit (SSIP)
    csrr t0, sip
    ori t0, t0, 1 << 1
    csrw sip, t0
    wfi
    ",
    inout("a0") a0,
    inout("a1") a1,
    inout("a2") a2,
    inout("a3") a3,
    inout("a4") a4,
    inout("a5") a5,
    inout("a6") a6,
    inout("a7") number,
    )
    }
    (a0, a1, a2, a3, a4, a5, a6)
}