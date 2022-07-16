use kernel_process::{ProcessState, ProcessContainer, Process};

use crate::HartLocals;

pub fn handle_syscall(process: &mut Process) {
	let args = kernel_syscall::get_syscall_args(process);
	println!("{:?}", args);
	match args.last().unwrap() {
		1 => {
			process.wake_on_paused.lock().state = ProcessState::Exited;
		}
		10 => {
			if args[1] != 0 {
				// Timer interrupt
				println!("{:?}", "a"); 
				let _for_time = args[1];
			} else {
				let external_interrupt_number = args[0];
				
                process.sleep();
                println!("SLEEP");
                
                let q = alloc::vec![process.waker()];
                HartLocals::current().interrupt_notifiers.borrow_mut()
                    .insert(external_interrupt_number, q);
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