use core::task::Waker;

use alloc::{sync::Arc, boxed::Box};

#[derive(Clone)]
pub struct MaybeWaker {
	wake_f: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl MaybeWaker {
	pub fn wake(self) -> bool {
		self.wake_by_ref()
	}
	pub fn wake_by_ref(&self) -> bool {
		(self.wake_f)()
	}
	pub fn noop() -> Self {
		Self {
			wake_f: Arc::new(|| {true})
		}
	}
}

impl Default for MaybeWaker {
	fn default() -> Self {
	    Self::noop()
	}
}

impl From<Arc<dyn Fn() -> bool>> for MaybeWaker {
	fn from(wake_f: Arc<dyn Fn() -> bool>) -> Self {
		Self {
			wake_f: unsafe { core::mem::transmute(wake_f) },
		}
	}
}
impl From<Waker> for MaybeWaker {
	fn from(wake_f: Waker) -> Self {
		Self {
			wake_f: Arc::new(move || {
				wake_f.wake_by_ref();
				true
			})
		}
	}
}


#[repr(transparent)]
struct MaybeWakerInternal {
	f: Box<dyn Fn() -> bool + Send + Sync>
}

impl alloc::task::Wake for MaybeWakerInternal {
	fn wake(self: Arc<Self>) {
		((*self).f)();
	}
}

impl Into<Waker> for MaybeWaker {
	fn into(self) -> Waker {
		Arc::new(MaybeWakerInternal {
			f: Box::new(move || {
				(self.wake_f)()
			})
		}).into()
	}
}

pub fn wake_all_that_are_ready(waker_list: impl Iterator<Item = MaybeWaker>) -> impl Iterator<Item = MaybeWaker> {
	waker_list.filter(MaybeWaker::wake_by_ref)
}

impl core::fmt::Debug for MaybeWaker {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("MaybeWaker {{ waker: {:p} }}", self.wake_f))
    }
}