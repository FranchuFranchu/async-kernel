use core::{
    future::Future,
    sync::atomic::{AtomicBool, Ordering},
    task::{Poll, Waker},
};

pub struct WaitForFunctionCallFuture<T: Unpin> {
    someone_waiting: AtomicBool,
    ready: spin::Mutex<Option<T>>,
    ready_waker: spin::Mutex<Option<Waker>>,
    waiting_waker: spin::Mutex<Option<Waker>>,
    enable: fn(),
}

impl<T: Unpin> Default for WaitForFunctionCallFuture<T> {
    fn default() -> Self {
        fn null_fn() {}
        Self::new(null_fn)
    }
}

impl<T: Unpin> WaitForFunctionCallFuture<T> {
    pub const fn new(enable: fn()) -> Self {
        Self {
            someone_waiting: AtomicBool::new(false),
            ready: spin::Mutex::new(None),
            ready_waker: spin::Mutex::new(None),
            waiting_waker: spin::Mutex::new(None),
            enable,
        }
    }

    pub fn wake(&self, data: T) {
        self.ready.lock().replace(data);
        let l = self.ready_waker.lock();
        if let Some(e) = &*l {
            e.wake_by_ref()
        };
    }

    pub fn wait(&self) -> Waiter<T> {
        Waiter(self)
    }

    fn poll(self: core::pin::Pin<&Self>, cx: &mut core::task::Context<'_>) -> core::task::Poll<T> {
        if let Some(data) = self.ready.lock().take() {
            return Poll::Ready(data);
        }
        self.someone_waiting.store(true, Ordering::Relaxed);

        let q = self.waiting_waker.lock();
        if let Some(e) = &*q {
            e.wake_by_ref()
        }

        self.ready_waker.lock().replace(cx.waker().clone());
        (self.enable)();
        Poll::Pending
    }
}

pub struct Waiter<'a, T: Unpin>(&'a WaitForFunctionCallFuture<T>);

impl<'a, T: Unpin> Future for Waiter<'a, T> {
    type Output = T;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        core::pin::Pin::new(self.0).poll(cx)
    }
}
