use alloc::{boxed::Box, collections::VecDeque, sync::Arc, vec::Vec};
use core::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    sync::atomic::AtomicBool,
    task::{Context, Waker},
};

use spin::Mutex;

pub fn run_neverending_future(mut future: impl Future<Output = !> + Unpin, idle: impl Fn()) -> ! {
    let ready_flag = AtomicBool::new(true);
    let waker = unsafe { crate::never_waker::new_waker(&ready_flag) };
    let mut context = Context::from_waker(&waker);
    loop {
        // If the ready flag is true, we can poll it again
        // otherwise, wait a bit.
        if ready_flag.swap(false, core::sync::atomic::Ordering::Relaxed) == true {
            let _ = Pin::new(&mut future).poll(&mut context);
        } else {
            idle()
        }
    }
}

pub type ExecutorFuture = dyn Future<Output = ()> + Send + Sync + Unpin;

struct Task {
    future: spin::Mutex<Box<ExecutorFuture>>,
}

impl Task {
    fn poll_future(self: Arc<Self>, waker: &Waker) -> Poll<()> {
        let mut lock = self.future.try_lock().unwrap();
        let x = Pin::new(&mut *lock).poll(&mut Context::from_waker(waker));
        x
    }
}

pub struct Executor {
    futures: Vec<Option<Arc<Task>>>,
    pub task_recv: ExecutorHandle,
}

impl Executor {
    pub fn new(futures: impl IntoIterator<Item = Box<ExecutorFuture>>) -> Self {
        Self {
            futures: futures
                .into_iter()
                .map(|future| {
                    Some(Arc::new(Task {
                        future: spin::Mutex::new(future),
                    }))
                })
                .collect(),
            task_recv: ExecutorHandle::default(),
        }
    }

    pub fn handle(&self) -> ExecutorHandle {
        self.task_recv.clone()
    }
}

pub async fn run_in_parallel(futures: impl IntoIterator<Item = Box<ExecutorFuture>>) {
    Executor::new(futures).await
}

use core::task::Poll;

impl Future for Executor {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        {
            let queue = self.task_recv.queue.clone();
            let mut lock = queue.lock();
            self.futures.extend(lock.drain(..).map(|s| {
                Some(Arc::new(Task {
                    future: spin::Mutex::new(s),
                }))
            }));
        }
        if self
            .futures
            .iter_mut()
            .map(|fut| {
                if let Some(internal) = fut {
                    match internal.clone().poll_future(cx.waker()) {
                        Poll::Ready(_) => {
                            *fut = None;
                            true
                        }
                        Poll::Pending => false,
                    }
                } else {
                    true
                }
            })
            .fold(true, |l, r| l && r)
        {
            return Poll::Ready(());
        } else {
            return Poll::Pending;
        }
    }
}

#[derive(Default, Clone)]
pub struct ExecutorHandle {
    queue: Arc<spin::Mutex<VecDeque<Box<ExecutorFuture>>>>,
}

#[repr(transparent)]
pub struct RawPtrExecutorHandle(*const spin::Mutex<VecDeque<Box<ExecutorFuture>>>);

impl ExecutorHandle {
    pub fn spawn(&self, future: Box<dyn Future<Output = ()> + Unpin>) {
        self.queue
            .lock()
            .push_back(unsafe { core::mem::transmute(future) })
    }

    pub fn from_raw(raw: RawPtrExecutorHandle) -> Self {
        // SAFETY: We can only get RawPtrExecutorHandle from these two functions.
        // from_raw only requires that this function is called with a pointer
        // returned from into_raw which always is the case
        unsafe {
            Self {
                queue: Arc::from_raw(raw.0),
            }
        }
    }

    pub fn into_raw(self) -> RawPtrExecutorHandle {
        RawPtrExecutorHandle(Arc::into_raw(self.queue))
    }
}
