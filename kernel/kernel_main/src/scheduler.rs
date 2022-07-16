//! The scheduler takes in tasks and
//! assigns them to harts.

use alloc::{boxed::Box, collections::VecDeque, sync::Arc};
use kernel_lock::shared::Mutex;


bitmask! {
	pub mask ExtensionRequirementSet: u32 where flags ExtensionRequirements {
		RV32I = 1,
		RV32E = 2,
		RV64I = 3,
		RV128I = 4,
		M = 1 << 3,
		A = 1 << 4,
		F = 1 << 5,
		D = 1 << 6,
		Q = 1 << 7,
		L = 1 << 8,
		C = 1 << 9,
		B = 1 << 10,
		J = 1 << 11,
		T = 1 << 12,
		P = 1 << 13,
		V = 1 << 14,
		K = 1 << 15,
		N = 1 << 16,
		H = 1 << 17,
		S = 1 << 18,
		Zam = 1 << 19,
		Ztso = 1 << 20,
	}
}

pub struct Requirements {
	std_extensions: ExtensionRequirementSet,
}

pub struct Task {
	pub function: Box<dyn FnMut()>,
	pub id: usize,
	/// Some tasks or processes require certain features, such as floating point or vector support
	/// to be run
	pub requirements: Requirements,
}

impl Task {
	pub fn new<F: FnMut()>(function: F) -> Task {
		Task {
			function,
			id: 0,
			requirements: Requirements { std_extensions: ExtensionRequirements::RV64I },
		}
	}
}

#[derive(Default)]
pub struct Scheduler {
	pub task_queue: VecDeque<Task>,
	pub sleeping_tasks: VecDeque<Task>,
	pub curr_id: usize,
}

impl Scheduler {
	pub fn take_task(&mut self) -> Option<Task> {
		// TODO requirement checking
		self.task_queue.pop_front()
	}
	pub fn push_task(&mut self, mut task: Task) {
		if task.id != 0 {
			self.curr_id = self.curr_id.wrapping_add(1);
			task.id = self.curr_id;
		}
		self.task_queue.push_back(task);
	}
	pub fn push_task_as_sleeping(&mut self, task: Task) {
		self.sleeping_tasks.push_back(task);
	}
	pub fn wake_up_task(&mut self, id: usize) {
		self.sleeping_tasks.push_back(task);
	}
}

#[derive(Clone, Default)]
pub struct SchedulerHandle {
	scheduler: Arc<Mutex<Scheduler>>
}