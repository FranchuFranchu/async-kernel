extern "C" {
    fn switch_to_supervisor_frame(a: *mut TrapFrame);
}

use kernel_cpu::{csr::XCAUSE_DESCRIPTION, read_scause, read_sip, read_sscratch, write_sip};
use kernel_trap_frame::TrapFrame;

pub fn make_interrupt_nonpending(cause: usize) {
    use kernel_cpu::csr::cause::*;
    match cause {
        SUPERVISOR_TIMER => {
            kernel_sbi::set_absolute_timer(u64::MAX);
        }
        SUPERVISOR_SOFTWARE => unsafe { write_sip(read_sip() & (!kernel_cpu::csr::SSIP)) },
        SUPERVISOR_EXTERNAL => {}
        _ => {
            println!("Unknown interrupt: {:?}", cause);
        }
    }
}

#[no_mangle]
pub unsafe fn trap_handler() {
    let cause = read_scause();
    let is_interrupt = cause >> (usize::BITS - 1) != 0;
    let cause = (cause << 1) >> 1;

    if is_interrupt {
        // Prevent interrupt from firing again after we exit
        make_interrupt_nonpending(cause);
    } else {
        println!("Error: {}.", XCAUSE_DESCRIPTION[cause]);
    }

    // Now, jump to the trap frame in sscratch
    let ctx = (*read_sscratch()).restore_context;
    if ctx != 0 {
        switch_to_supervisor_frame(ctx as *mut _);
    }
}
