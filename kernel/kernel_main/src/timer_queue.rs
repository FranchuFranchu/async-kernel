use alloc::rc::Rc;

use kernel_cpu::read_time;

use crate::{local_notify::Notify, HartLocals};

pub async fn task_schedule_time_interrupts() {
    let hart_locals = HartLocals::current();
    loop {
        let timer_queue = hart_locals.timer_queue.borrow_mut();
        if let Some((minimum_time, _)) = timer_queue.iter().min_by(|lhs, rhs| lhs.0.cmp(&rhs.0)) {
            let minimum_time = *minimum_time;
            drop(timer_queue);
            println!("Timer queue");
            kernel_sbi::set_absolute_timer(minimum_time);
        }
        println!("{:?}", "minimum etime");
        hart_locals.timer_scheduled_notify.tx_ready().await;
    }
}

pub async fn task_handle_time_interrupts() {
    let hart_locals = HartLocals::current();
    loop {
        {
            let timer_queue = hart_locals.timer_queue.borrow_mut();
            if let Some((_minimum_time, notify)) =
                timer_queue.iter().min_by(|lhs, rhs| lhs.0.cmp(&rhs.0))
            {
                let notify = notify.clone();
                drop(timer_queue);
                notify.wake_rx()
            }
        }

        hart_locals.timer_happened_notify.tx_ready().await;
    }
}

pub fn schedule_interrupt_for(_time: u64) -> Rc<Notify> {
    let hart_locals = HartLocals::current();

    let notify = Rc::new(Notify::default());
    hart_locals
        .timer_queue
        .borrow_mut()
        .push_back((0, notify.clone()));
    hart_locals.timer_scheduled_notify.try_wake_rx();
    notify
}

pub async fn wait_until_time(time: u64) {
    schedule_interrupt_for(time).tx_ready().await;
    println!("{:?}", "finished time");
}

pub async fn wait_for(relative_time: u64) {
    schedule_interrupt_for(relative_time + read_time())
        .tx_ready()
        .await;
}
