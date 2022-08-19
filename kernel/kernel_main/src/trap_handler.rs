extern "C" {
    fn switch_to_supervisor_frame(a: *mut TrapFrame);
}

use core::{sync::atomic::Ordering};

use kernel_chip_drivers::plic::Plic0;
use kernel_cpu::{csr::{XCAUSE_DESCRIPTION, SSIE}, read_scause, read_sip, read_sscratch, write_sip, read_stval, read_sstatus, write_sie, read_sie};
use kernel_paging::{Sv39, EntryBits};
use kernel_trap_frame::TrapFrame;
use kernel_process::Process;

use crate::{GAP, HartLocals, phys_to_virt, virt_to_phys, syscall::handle_syscall, loop_forever_black_box};

pub fn handle_interrupt(mut process: Option<&mut Process>, cause: usize) {
    use kernel_cpu::csr::cause::*;
    let cause = if process.is_none() { 
        // If the process is None, we only care about making the interrupt nonpending
        read_sip().log2() as usize
    } else { 
        cause as usize 
    };
    match cause {
        SUPERVISOR_TIMER => {
            kernel_sbi::set_absolute_timer(u64::MAX);
        }
        SUPERVISOR_SOFTWARE => unsafe { 
            handle_syscall(process.as_mut().unwrap());
            write_sip(read_sip() & (!kernel_cpu::csr::SSIP)) 
        },
        SUPERVISOR_EXTERNAL => {
            let plic = Plic0::new_with_addr(GAP.load(Ordering::Relaxed) + 0x0c00_0000);
            
            let id = plic.claim_highest_priority();
            
            if let Some(e) = HartLocals::current().interrupt_notifiers.borrow_mut().remove(&(id as usize)) {
                for waker in e {
                    waker.wake();
                }
            }
            
            if id == 10 {
                let c = unsafe { ((0x1000_0000 as *const u8).add(GAP.load(Ordering::Relaxed))).read_volatile() };
                println!("Char {:?}", c as char);
            }
            
            
            plic.complete(id);
        }
        _ => {
            println!("Unknown interrupt: {:?}", cause);
        }
    };
    if let Some(p) = process {
        p.trap_frame.sie &= (!read_sip()) & 0x222;
    } else {
        unsafe { write_sie((!read_sip()) & 0x222) };
    }
}

/// Should be executed after coming back from a process
pub fn handle_come_back_from_process(process: Option<&mut Process>) {
    let cause = read_scause();
    let _is_interrupt = cause >> (usize::BITS - 1) != 0;
    let cause = (cause << 1) >> 1;
    
    handle_interrupt(process, cause);
}


#[no_mangle]
pub unsafe fn trap_handler() {
    let cause = read_scause();
    let is_interrupt = cause >> (usize::BITS - 1) != 0;
    let cause = (cause << 1) >> 1;
    
    
    let trap_frame = read_sscratch().as_mut().unwrap();
    trap_frame.sie = 0;
    let mut switch_to_trap_frame = ((*read_sscratch()).restore_context as *const TrapFrame as *mut TrapFrame).as_mut();
    if let Some(e) = &mut switch_to_trap_frame {
        if is_interrupt {
            e.sie = 0;
        }
    }
    let _next_trap_frame = switch_to_trap_frame.as_ref().unwrap_or(&trap_frame);
    
    
    if is_interrupt {
        // Prevent interrupt from firing again after we exit
        //make_interrupt_nonpending(cause, trap_frame);
        
        /*if next_trap_frame.sie != 0 {
            println!("{:?}", switch_to_trap_frame);
        }*/
    } else {
        println!("Error: {}.", XCAUSE_DESCRIPTION[cause]);
        
        let tval = read_stval();
        println!("SATP: {:#x}", trap_frame.satp);
        let sv39 = Sv39::from_satp(trap_frame.satp, phys_to_virt, virt_to_phys);
        let is_supervisor = (read_sstatus() & (1 << 8)) != 0;
        let permissions = 
            if !is_supervisor { EntryBits::USER as u8 } else { 0 };
        if tval != 0 {
            println!("{:?}", "Simulating page access:");
            println!("{:#x} -> {:?}", tval, sv39.query_permissions(tval, permissions));
        } else {
            println!("TVAL = 0");
        }
        
        loop_forever_black_box();
        unsafe { kernel_process::switch_to_supervisor_frame(read_sscratch()) }
        loop {}
    }
    
    if let Some(context) = switch_to_trap_frame {
        switch_to_supervisor_frame(context as *const _ as *mut _)
    }

}
