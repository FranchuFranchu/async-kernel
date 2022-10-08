use alloc::{
    boxed::Box,
    collections::VecDeque,
    rc::{Rc, Weak},
    vec::Vec,
};
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

use kernel_lock::shared_refcell::RefCell;

use crate::non_send_waker::{RcWake, RcWakeInto};

type FutureType = dyn Future<Output = ()> + Unpin;

struct LocalWaker {
    executor: Weak<RefCell<VecDeque<usize>>>,
    wakers: Weak<RefCell<Vec<Waker>>>,
    index: usize,
}

impl RcWake for LocalWaker {
    fn rc_wake_by_ref(self: &Rc<Self>) {
        self.executor
            .upgrade()
            .unwrap()
            .borrow_mut()
            .push_back(self.index);

        self.wakers
            .upgrade()
            .unwrap()
            .borrow_mut()
            .drain(..)
            .for_each(|s| s.wake());
    }
}

#[derive(Default)]
pub struct LocalExecutor {
    this: Weak<RefCell<Self>>,
    tasks: Vec<Option<Box<FutureType>>>,
    wakers: Rc<RefCell<Vec<Waker>>>,
    wake_queue: Rc<RefCell<VecDeque<usize>>>,
    task_queue: Rc<RefCell<VecDeque<Box<FutureType>>>>,
}

#[derive(Clone)]
pub struct LocalExecutorHandle {
    pub executor: Rc<RefCell<LocalExecutor>>,
    pub queue: Rc<RefCell<VecDeque<Box<FutureType>>>>,
}

impl LocalExecutorHandle {
    pub fn spawn(&self, future: Box<FutureType>) {
        self.queue.borrow_mut().push_back(future);
    }
}

impl LocalExecutor {
    pub fn new() -> Rc<RefCell<Self>> {
        let this = Rc::new(RefCell::new(Self::default()));
        this.borrow_mut().this = Rc::downgrade(&this);
        this
    }

    pub fn handle(&self) -> LocalExecutorHandle {
        LocalExecutorHandle {
            executor: self.this.upgrade().unwrap(),
            queue: self.task_queue.clone(),
        }
    }

    fn poll_future(&mut self, index: usize) {
        if let Some(task) = self.tasks.get_mut(index) {
            if task.is_none() {
                return;
            }
            let waker = Rc::new(LocalWaker {
                executor: Rc::downgrade(&self.wake_queue),
                wakers: Rc::downgrade(&self.wakers),
                index,
            })
            .into_waker();
            let mut cx = Context::from_waker(&waker.waker);
            if Pin::new(task.as_mut().unwrap()).poll(&mut cx).is_ready() {
                task.take();
            }
        }
    }

    fn check_if_done(&mut self) -> bool {
        !self.tasks.iter().any(|s| s.is_some())
    }

    fn wake_pending(&mut self) -> bool {
        let q: VecDeque<usize> = core::mem::take(&mut *self.wake_queue.borrow_mut());

        // Remove duplicates
        let is_empty = q.is_empty();
        let mut seen = Vec::with_capacity(q.capacity());
        for i in q.iter() {
            if seen.contains(i) {
                continue;
            }
            seen.push(*i);
            self.poll_future(*i);
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

impl Future for LocalExecutor {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        self.wakers.borrow_mut().push(context.waker().clone());
        if self.check_if_done() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl Future for LocalExecutorHandle {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context) -> Poll<()> {
        self.executor.borrow_mut().poll_all();
        for i in self.queue.borrow_mut().drain(..) {
            self.executor.borrow_mut().push_task(i);
        }
        let r = Pin::new(&mut *self.executor.borrow_mut()).poll(context);
        if !self.executor.borrow_mut().wake_pending() {
            for i in self.queue.borrow_mut().drain(..) {
                self.executor.borrow_mut().push_task(i);
            }
            Poll::Pending
        } else {
            r
        }
    }
}
