use core::task::Waker;

use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use kernel_lock::spin::RwLock;
use kernel_lock::spin::Mutex;
use kernel_paging::PartialMapping;
use kernel_process::{Process, ProcessContainer, ProcessState};
use kernel_syscall::get_syscall_args;
use kernel_util::boxed_slice_with_alignment;
use kernel_util::maybe_waker::MaybeWaker;
use kernel_util::maybe_waker::wake_all_that_are_ready;

use crate::HartLocals;
use crate::phys_to_virt;

#[derive(Clone, Debug)]
pub enum InflightBufferMode {
    Own,
    Borrow,
    BorrowMut,
    Copied,
}

#[derive(Debug)]
pub enum InflightBufferContents {
    Mapped(PartialMapping),
    Copied(alloc::boxed::Box<[u8]>),
}

#[derive(Debug)]
pub struct InflightBuffer {
    mode: InflightBufferMode,
    contents: InflightBufferContents,
    claim: Waker,
}

#[derive(Default, Debug)]
pub struct BufferQueue {
    waiting_wakers: VecDeque<MaybeWaker>,
    waiting_buffers: VecDeque<InflightBuffer>,
}

impl BufferQueue {
    fn send_buffer(&mut self, buffer: InflightBuffer) {
        self.waiting_buffers.push_back(buffer);
        self.waiting_wakers = wake_all_that_are_ready(self.waiting_wakers.drain(..)).collect();
    }
    
    /// Attempt to take a buffer from the queue.
    /// `f` is a function that takes in a buffer, and returns either an Ok with some custom data, or gives back the buffer as an error
    fn try_take_buffer<T>(&mut self, mut f: impl FnMut(&mut Self, InflightBuffer) -> Result<T, InflightBuffer>, waker: MaybeWaker) -> Option<T> {
        while let Some(buffer) = self.waiting_buffers.pop_front() {
            let waker = buffer.claim.clone();
            match f(self, buffer) {
                Ok(data) => {
                    waker.wake();
                    return Some(data)
                },
                Err(buffer) => {
                    // Put the buffer back in its place and continue on to the next buffer in the queue
                    self.waiting_buffers.push_front(buffer);       
                }
            }
        }
        // No buffers were consumed. Register the waker.
        self.waiting_wakers.push_back(waker);
        None
    }
    
    fn buffer_amount(&self) -> usize {
        self.waiting_buffers.len()
    }
    
    fn add_waker(&mut self, waker: MaybeWaker) {
        self.waiting_wakers.push_back(waker);
    }
}


pub static INFLIGHT_BUFFERS: RwLock<BTreeMap<usize, Mutex<BufferQueue>>> = RwLock::new(BTreeMap::new());

pub fn handle_syscall(process: &mut Process) {
    process.trap_frame.pc += 2;
    let args = kernel_syscall::get_syscall_args(process);
    let syscall_number = *args.last().unwrap();
    
    match syscall_number {
        1 => {
            process.wake_on_paused.lock().state = ProcessState::Exited;
        }
        2 => {
            // Enable a future (get notified when it's done).
            let future_id = args[0];
            
            process.wake_on_paused.lock().enable_source(future_id as u64);
        }
        3 => {
            process.sleep();
        }
        10 => {
            if args[1] != 0 {
                // Timer interrupt
                let _for_time = args[1];
            } else {
                let external_interrupt_number = args[0];
                
                let (waker, future_id) = process.waker();

                let mut interrupt_wakers = HartLocals::current().interrupt_notifiers.borrow_mut();
                if let Some(vec) = interrupt_wakers.get_mut(&external_interrupt_number) {
                    vec.push(waker.into());
                } else {
                    interrupt_wakers.insert(external_interrupt_number, alloc::vec![waker.into()]);
                }
                
                let args = kernel_syscall::get_syscall_args(process);
                
                args[0] = future_id as usize;
            }
        }
        0x10 | 0x11 | 0x12 | 0x13 => {
            // 0x10 - Move out a buffer.
            // 0x11 - Borrow out a buffer.
            // 0x12 - Mutably borrow out a buffer.
            // 0x13 - Copy out a buffer.
            
            // Returns a future that will be ready when it's Ready
            
            let virtual_start_addr = args[0];
            let virtual_size = args[1];
            let target_buffer_queue = args[2];
            
            drop(args);
            println!("{:?}", process.trap_frame.satp);
            let mut process_page_table = unsafe { crate::paging_from_satp(process.trap_frame.satp) };
            let partial_mapping = unsafe {
                process_page_table.copy_partial_mapping(virtual_start_addr, virtual_size)
            };
            
            let (waker, future_id) = process.waker();
            let buffer = if syscall_number == 0x13 {
                println!("{:?}", partial_mapping);
                let collected_data = unsafe {
                    partial_mapping.read_iter(crate::phys_to_virt).fold(alloc::vec::Vec::new(), |mut accum, new| {
                        accum.extend_from_slice(new);
                        accum
                    })
                };
                let collected_data = collected_data.into_boxed_slice();
                
                InflightBuffer {
                    contents: InflightBufferContents::Copied(collected_data),
                    mode: InflightBufferMode::Copied,
                    claim: waker.into(),
                }
            } else {
                assert!(virtual_start_addr & 4095 == 0);
                assert!(virtual_size & 4095 == 0);
                println!("{:?}", partial_mapping);
                // Now that we have a partial mapping, we'll choose what to do with it
                if syscall_number == 0x10 || syscall_number == 0x12 {
                    // The buffer is being moved or borrowed out. Therefore we want to erase the mapping from the source process's page table
                    process_page_table.map(virtual_start_addr, virtual_start_addr, virtual_size, 0);
                }
                
                let mode = match syscall_number {
                    0x10 => InflightBufferMode::Own,
                    0x11 => InflightBufferMode::Borrow,
                    0x12 => InflightBufferMode::BorrowMut,
                    _ => unreachable!(),
                };
                
                InflightBuffer {
                    contents: InflightBufferContents::Mapped(partial_mapping),
                    mode,
                    claim: waker.into(),
                }
            };
                
            let mut lock = INFLIGHT_BUFFERS.write();
            let buffer_queue = if let Some(queue) = lock.get(&target_buffer_queue) {
                queue
            } else {
                lock.insert(target_buffer_queue, Mutex::new(Default::default()));
                lock.get(&target_buffer_queue).unwrap()
            };
            
            buffer_queue.lock().send_buffer(buffer);
            
            println!("{:?}", lock);
            
            let args = get_syscall_args(process);
            args[0] = future_id as usize;
        }
        0x20 | 0x21 => {
            // (virtual_addr, max_size, queue_id) -> (status, real_size, remaining_buffers, future_id (or 0))
            
            // 0x20 is move the page table mapping into the virtual area specified. This means 
            // that the area may not actually be mapped!
            // 0x21 is copy the contents of the data there into the virtual area specified.
            let map_to_virtual_address = args[0];
            let maximum_size = args[1];
            let source_buffer_queue = args[2];
            
            drop(args);
            
            let mut closure = |virtual_addr, max_size, queue_id| -> (usize, usize, usize) {
                let mut process_page_table = unsafe { crate::paging_from_satp(process.trap_frame.satp) };
                let mut lock = INFLIGHT_BUFFERS.read();
                if let Some(queue_mutex) = lock.get(&queue_id) {
                    let mut queue = queue_mutex.lock();
                    queue.try_take_buffer(|queue, buffer| {
                        match &buffer.contents {
                            InflightBufferContents::Mapped(partial_mapping) => {
                                let size = partial_mapping.size();
                                let mode = buffer.mode.clone();
                    
                                if size <= maximum_size {
                                    process_page_table.paste_partial_mapping(virtual_addr, &partial_mapping, 7);
                                } else {
                                    return Err(buffer);
                                }
                                
                                Ok((match mode {
                                    InflightBufferMode::Own => 1,
                                    InflightBufferMode::Borrow => 2,
                                    InflightBufferMode::BorrowMut => 3,
                                    InflightBufferMode::Copied => 4,
                                }, size, queue.buffer_amount()))
                            }
                            InflightBufferContents::Copied(buffer_data) => {
                                let size = buffer_data.len();
                                if size <= maximum_size {
                                    unsafe {
                                        let mut pm = process_page_table.copy_partial_mapping(virtual_addr, max_size);
                                        pm.trim_size_to(size);
                                        pm.overwrite_contents_with(buffer_data.iter().copied(), phys_to_virt);
                                    };
                                } else {
                                    return Err(buffer);
                                }
                                
                                Ok((4, size, queue.buffer_amount()))
                            }
                        }
                    }, process.waker().0.into()).unwrap_or((0, 0, 0))
                } else {
                    (0, 0, 0)
                }   
            };
            let (status, real_size, remaining_buffers) = closure(map_to_virtual_address, maximum_size, source_buffer_queue);
            
            let args = kernel_syscall::get_syscall_args(process);
            args[0] = status;
            args[1] = real_size;
            args[2] = remaining_buffers;
            
            if status == 0 {
                
            }
        },
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
