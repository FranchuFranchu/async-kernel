use alloc::{boxed::Box, string::String, sync::Arc};

use kernel_cpu::{read_satp, read_sscratch, Registers};
use kernel_lock::shared::Mutex;
use kernel_trap_frame::TrapFrame;
use kernel_util::boxed_slice_with_alignment;

extern "C" {
    fn store_to_trap_frame_and_run_function(a: *mut TrapFrame, b: usize, c: usize);
    fn switch_to_supervisor_frame(a: *mut TrapFrame);
}

#[derive(Default)]
pub struct Process {
    pub is_supervisor: bool,
    pub trap_frame: Box<TrapFrame>,
    pub name: Option<String>,
    pub kernel_allocated_stack: Option<Box<[u8]>>,
}

impl Process {
    pub fn switch_to_and_come_back(&mut self) {
        unsafe extern "C" fn run(trap_frame: *mut TrapFrame, _: usize, a2: &mut Process) {
            a2.trap_frame.restore_context = trap_frame as usize;
            let frame: &TrapFrame = &a2.trap_frame;
            switch_to_supervisor_frame(frame as *const _ as *mut _);
        }
        let mut my_trap_frame = Box::new(TrapFrame::zeroed_interrupt_context());
        unsafe { my_trap_frame.inherit_from(read_sscratch().as_mut().unwrap()) };

        my_trap_frame.satp = read_satp();
        my_trap_frame.kernel_satp = read_satp();

        unsafe {
            store_to_trap_frame_and_run_function(
                &mut *my_trap_frame as *mut _,
                run as usize,
                self as *mut _ as usize,
            )
        }
    }

    pub fn new_supervisor<C: FnOnce(&mut Process)>(
        constructor: C,
        function: fn(),
    ) -> Arc<Mutex<Self>> {
        let mut this = Self {
            is_supervisor: true,
            kernel_allocated_stack: Some(boxed_slice_with_alignment(4096 * 2, 4096, &0)),
            ..Default::default()
        };

        this.trap_frame.general_registers[Registers::Sp as usize] =
            this.kernel_allocated_stack.as_ref().unwrap().as_ptr() as usize;
        this.trap_frame.pc = function as usize;
        this.trap_frame.interrupt_stack = 0x8700_0000 as usize;
        let this_frame = unsafe { read_sscratch().as_mut().unwrap() };
        this.trap_frame.satp = this_frame.satp;
        this.trap_frame.kernel_satp = this_frame.satp;

        constructor(&mut this);

        let this = Arc::new(Mutex::new(this));

        this
    }
}
