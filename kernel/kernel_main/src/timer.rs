use kernel_cpu::read_time;

pub fn set_relative_timer(time: u64) {
    kernel_sbi::set_absolute_timer(read_time().saturating_add(time));
}
