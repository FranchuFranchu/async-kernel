use alloc::{
    boxed::Box,
    collections::VecDeque,
    sync::{Arc, Weak},
    task::Wake,
    vec::Vec,
};
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

use spin::Mutex;

type FutureType = dyn Future<Output = ()> + Unpin + Send;

struct SendWaker {
    executor: Weak<Mutex<VecDeque<usize>>>,
    wakers: Weak<Mutex<Vec<Waker>>>,
    index: usize,
}

impl Wake for SendWaker {
    fn wake(self: Arc<Self>) {
        self.executor
            .upgrade()
            .unwrap()
            .lock()
            .push_back(self.index);

        self.wakers
            .upgrade()
            .unwrap()
            .lock()
            .drain(..)
            .for_each(|s| s.wake());
    }
}

#[derive(Default)]
pub struct SendExecutor {
    this: Weak<Mutex<Self>>,
    tasks: Vec<Option<Box<FutureType>>>,
    wakers: Arc<Mutex<Vec<Waker>>>,
    wake_queue: Arc<Mutex<VecDeque<usize>>>,
    task_queue: Arc<Mutex<VecDeque<Box<FutureType>>>>,
}

#[derive(Clone)]
pub struct SendExecutorHandle {
    pub executor: Arc<Mutex<SendExecutor>>,
    pub queue: Arc<Mutex<VecDeque<Box<FutureType>>>>,
}

impl SendExecutorHandle {
    pub fn spawn(&self, future: Box<FutureType>) {
        self.queue.lock().push_back(future);
    }
}

impl SendExecutor {
    pub fn new() -> Arc<Mutex<Self>> {
        let this = Arc::new(Mutex::new(Self::default()));
        this.lock().this = Arc::downgrade(&this);
        this
    }

    pub fn handle(&self) -> SendExecutorHandle {
        SendExecutorHandle {
            executor: self.this.upgrade().unwrap(),
            queue: self.task_queue.clone(),
        }
    }

    fn poll_future(&mut self, index: usize) {
        if let Some(task) = self.tasks.get_mut(index) {
            if task.is_none() {
                return;
            }
            let waker = Arc::new(SendWaker {
                executor: Arc::downgrade(&self.wake_queue),
                wakers: Arc::downgrade(&self.wakers),
                index,
            })
            .into();
            let mut cx = Context::from_waker(&waker);
            if Pin::new(task.as_mut().unwrap()).poll(&mut cx).is_ready() {
                task.take();
            }
        }
    }

    fn check_if_done(&mut self) -> bool {
        !self.tasks.iter().any(|s| s.is_some())
    }

    fn wake_pending(&mut self) -> bool {
        let q: Vec<usize> = self.wake_queue.lock().drain(..).collect();
        let is_empty = q.is_empty();
        for i in q {
            self.poll_future(i);
        }
        is_empty
    }

    fn poll_all(&mut self) {
        for i in 0..self.tasks.len() {
            self.poll_future(i);
        }
    }

    pub fn push_task(&mut self, task: Box<FutureType>) -> usize {
        let index = if let Some(slot) = self
            .tasks
            .iter_mut()
            .enumerate()
            .find(|(_, val)| val.is_none())
        {
            slot.1.replace(task);
            slot.0
        } else {
            self.tasks.push(Some(task));
            self.tasks.len() - 1
        };
        self.poll_future(index);
        index
    }
}

impl Future for SendExecutor {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        self.wakers.lock().push(context.waker().clone());
        if self.check_if_done() {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

impl Future for SendExecutorHandle {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        self.executor.lock().poll_all();
        for i in self.queue.lock().drain(..) {
            self.executor.lock().push_task(i);
        }
        let r = Pin::new(&mut *self.executor.lock()).poll(context);
        if !self.executor.lock().wake_pending() {
            for i in self.queue.lock().drain(..) {
                self.executor.lock().push_task(i);
            }
            Poll::Pending
        } else {
            r
        }
    }
}
