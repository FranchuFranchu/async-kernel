use core::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

pub struct Notify {
    transmit_waker: RefCell<(bool, Option<Waker>)>,
    receive_waker: RefCell<(bool, Option<Waker>)>,
}

impl Default for Notify {
    fn default() -> Self {
        Self {
            transmit_waker: RefCell::new((false, None)),
            receive_waker: RefCell::new((false, None)),
        }
    }
}

impl Notify {
    pub fn wake_tx(&self) {
        self.try_wake_tx().unwrap()
    }

    pub fn try_wake_tx(&self) -> Option<()> {
        let mut borrow = self.transmit_waker.borrow_mut();
        borrow.0 = true;
        borrow.1.as_ref()?.wake_by_ref();
        Some(())
    }

    fn register_tx(&self, transmit_waker: Waker) {
        self.transmit_waker.borrow_mut().1.replace(transmit_waker);
    }

    pub fn wake_rx(&self) {
        self.try_wake_rx().unwrap()
    }

    pub fn try_wake_rx(&self) -> Option<()> {
        let mut borrow = self.receive_waker.borrow_mut();
        borrow.0 = true;
        borrow.1.as_ref()?.wake_by_ref();
        Some(())
    }

    fn register_rx(&self, receive_waker: Waker) {
        self.receive_waker.borrow_mut().1.replace(receive_waker);
        self.try_wake_tx();
    }

    pub fn rx_ready(&self) -> WaitForRxFuture {
        WaitForRxFuture(self)
    }

    pub fn tx_ready(&self) -> WaitForTxFuture {
        WaitForTxFuture(self)
    }
}

pub struct WaitForTxFuture<'notify>(&'notify Notify);

impl<'notify> Future for WaitForTxFuture<'notify> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<()> {
        if self.0.receive_waker.borrow().0 {
            self.0.receive_waker.borrow_mut().0 = false;
            Poll::Ready(())
        } else {
            self.0.register_rx(cx.waker().clone());
            Poll::Pending
        }
    }
}

pub struct WaitForRxFuture<'notify>(&'notify Notify);

impl<'notify> Future for WaitForRxFuture<'notify> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<()> {
        if self.0.transmit_waker.borrow().0 {
            Poll::Ready(())
        } else {
            self.0.register_tx(cx.waker().clone());
            Poll::Pending
        }
    }
}
