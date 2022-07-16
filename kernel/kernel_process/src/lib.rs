#![no_std]

extern crate alloc;

use core::{task::{Waker, Poll}, future::Future, borrow::BorrowMut, sync::atomic::AtomicBool};

use alloc::{boxed::Box, string::String, sync::{Arc, Weak}, task::Wake, vec::Vec};

use kernel_cpu::{read_satp, read_sscratch, Registers, write_sscratch};
use kernel_lock::shared::Mutex;
use kernel_trap_frame::TrapFrame;
use kernel_util::boxed_slice_with_alignment;

extern "C" {
    fn store_to_trap_frame_and_run_function(a: *mut TrapFrame, b: usize, c: usize);
    pub fn switch_to_supervisor_frame(a: *mut TrapFrame);
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProcessState {
    Running,
    Paused,
    Yielded,
    Exited,
}

impl Default for ProcessState {
    fn default() -> Self {
        Self::Paused
    }
}

#[derive(Default)]
pub struct Process {
    pub is_supervisor: bool,
    pub trap_frame: Box<TrapFrame>,
    pub name: Option<String>,
    pub kernel_allocated_stack: Option<Box<[u8]>>,
    pub wake_on_paused: Arc<Mutex<ProcessWakerStruct>>,
    pub this: ProcessContainerWeak,
}

#[derive(Default, Debug)]
pub struct ProcessWakerStruct {
    wakers: Vec<Waker>,
    pub state: ProcessState,
}

impl ProcessWakerStruct {
    fn take_wakers(&mut self) -> Vec<Waker> {
        self.state = ProcessState::Paused;
        core::mem::take(&mut self.wakers)
    }
    fn wake_up(this: &Mutex<Self>) {
        let wakers = this.lock().take_wakers();
        wakers.into_iter().for_each(|s| s.wake())
    }
    fn add_waker(&mut self, waker: Waker) {
        self.wakers.push(waker);
    }
}

impl Process {
    pub fn switch_to_and_come_back(&mut self) {
        unsafe { kernel_cpu::write_sie(0) };
        let old_sscratch = read_sscratch();
        unsafe extern "C" fn run(trap_frame: *mut TrapFrame, _: usize, a2: &mut Process) {
            a2.trap_frame.restore_context = trap_frame as usize;
            let frame: &TrapFrame = &a2.trap_frame;
            assert!(frame.satp != 0);
            switch_to_supervisor_frame(frame as *const _ as *mut _);
        }
        let mut my_trap_frame = Box::new(TrapFrame::zeroed_interrupt_context());
        unsafe { my_trap_frame.inherit_from(read_sscratch().as_mut().unwrap()) };

        my_trap_frame.satp = read_satp();
        my_trap_frame.kernel_satp = read_satp();
        
        self.wake_on_paused.lock().state = ProcessState::Running;
        
        unsafe {
            store_to_trap_frame_and_run_function(
                &mut *my_trap_frame as *mut _,
                run as usize,
                self as *mut _ as usize,
            );
        }
        self.wake_on_paused.lock().state = ProcessState::Paused;
        unsafe { write_sscratch(old_sscratch as _) };
    }

    pub fn new_supervisor<C: FnOnce(&mut Process)>(
        constructor: C,
        function: fn(),
    ) -> Arc<Mutex<Self>> {
        let mut this = Self {
            is_supervisor: true,
            kernel_allocated_stack: Some(boxed_slice_with_alignment(4096 * 8, 4096, &0)),
            ..Default::default()
        };

        this.trap_frame.general_registers[Registers::Sp as usize] =
            this.kernel_allocated_stack.as_ref().unwrap().as_ptr() as usize;
        this.trap_frame.pc = function as usize;
        let this_frame = unsafe { read_sscratch().as_mut().unwrap() };
        
        
        this.trap_frame.inherit_from(&this_frame);
        this.trap_frame.satp = this_frame.satp;
        this.trap_frame.kernel_satp = this_frame.satp;

        constructor(&mut this);

        let this = Arc::new(Mutex::new(this));
        
        this.lock().this = Arc::downgrade(&this);

        this
    }
    
    
    pub fn waker(&mut self) -> Waker {
        Arc::new(ProcessWaker(self.wake_on_paused.clone())).into()
    }
    
    pub fn wait_until_woken(&mut self) -> WaitUntilReady {
        WaitUntilReady(self.wake_on_paused.clone(), Default::default())
    }
    
    pub fn sleep(&mut self) {
        self.wake_on_paused.lock().state = ProcessState::Yielded;
    }
}


pub struct ProcessWaker(Arc<Mutex<ProcessWakerStruct>>);

impl Wake for ProcessWaker {
    fn wake(self: Arc<Self>) {
        ProcessWakerStruct::wake_up(&self.0)
    }
}

use core::sync::atomic::Ordering;

#[derive(Default)]
pub struct MakeTrueWaker(AtomicBool);

impl Wake for MakeTrueWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.store(true, Ordering::Release)
    }
}

impl MakeTrueWaker {
    fn was_woken(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

pub struct WaitUntilReady(Arc<Mutex<ProcessWakerStruct>>, Arc<MakeTrueWaker>);

impl Future for WaitUntilReady {
    type Output = ();

    fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<Self::Output> {
        let mut lock = self.0.lock();
        if self.1.was_woken() {
            Poll::Ready(())
        } else if let ProcessState::Yielded = lock.state {
            lock.add_waker(self.1.clone().into());
            lock.add_waker(cx.waker().clone());
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}


pub type ProcessContainer = Arc<Mutex<Process>>;
pub type ProcessContainerWeak = Weak<Mutex<Process>>;