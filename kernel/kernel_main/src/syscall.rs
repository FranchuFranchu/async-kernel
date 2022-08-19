use kernel_process::{ProcessState, ProcessContainer, Process};

use crate::HartLocals;

pub fn handle_syscall(process: &mut Process) {
	process.trap_frame.pc += 2;
	let args = kernel_syscall::get_syscall_args(process);
	match args.last().unwrap() {
		1 => {
			process.wake_on_paused.lock().state = ProcessState::Exited;
		}
		10 => {
			if args[1] != 0 {
				// Timer interrupt 
				let _for_time = args[1];
			} else {
				let external_interrupt_number = args[0];
				
                process.sleep();
                
                let w = process.waker();
                let mut interrupt_wakers = HartLocals::current().interrupt_notifiers.borrow_mut();
                if let Some(vec) = interrupt_wakers.get_mut(&external_interrupt_number) {
                	vec.push(w);
                } else {
                	interrupt_wakers.insert(external_interrupt_number, alloc::vec![w]);
                }
            }
		}
		_ => {
			panic!("Unknown syscall {}!", args.last().unwrap());
		}
	}
}

pub async fn wait_until_process_is_woken(process: &ProcessContainer) {
	let fut = {
		let mut lock = process.lock();
		if lock.wake_on_paused.lock().state != ProcessState::Yielded {
			return;
		} else {
			lock.wait_until_woken()
		}
	};
	fut.await;
}