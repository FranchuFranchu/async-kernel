#![no_std]

/// A pointer to this struct is placed in sscratch
#[derive(Default, Clone, Debug)] // No copy because they really shouldn't be copied and used without changing the PID
#[repr(C)]
pub struct TrapFrame {
    pub general_registers: [usize; 32],
    pub pc: usize,     // 32
    pub hartid: usize, // 33
    pub pid: usize,    // 34
    pub interrupt_stack: usize, /* 35. This may be shared between different processes executing
                        * the same hart */
    pub flags: usize,           // 36
    pub satp: usize,            // 37
    pub kernel_satp: usize,     // 38
    pub hart_locals: usize,     // 39
    pub process_raw_ptr: usize, // 40
    pub restore_context: usize, // 41
}

impl TrapFrame {
    pub const fn zeroed() -> Self {
        Self {
            general_registers: [0; 32],
            hartid: 0,
            pid: 0,
            pc: 0,
            interrupt_stack: 0,
            flags: 0,
            satp: 0,
            kernel_satp: 0,
            hart_locals: 0,
            process_raw_ptr: 0,
            restore_context: 0,
        }
    }

    pub const fn zeroed_interrupt_context() -> Self {
        let mut this = Self::zeroed();
        this.flags = 1;
        return this;
    }

    // Inherit hartid, interrupt_stack, and flags from the other trap frame
    pub fn inherit_from(&mut self, other: &TrapFrame) -> &mut TrapFrame {
        self.hartid = other.hartid;
        self.hart_locals = other.hart_locals;
        self.interrupt_stack = other.interrupt_stack;
        self.flags = other.flags;
        self.satp = other.satp;
        self
    }

    pub fn is_interrupt_context(&self) -> bool {
        self.flags & 1 != 0
    }

    pub fn has_trapped_before(&self) -> bool {
        self.flags & 2 != 0
    }

    pub fn is_double_faulting(&self) -> bool {
        self.flags & 4 != 0
    }

    pub fn is_in_fault_trap(&self) -> bool {
        self.flags & 8 != 0
    }

    pub fn set_interrupt_context(&mut self) {
        self.flags |= 1
    }

    pub fn set_trapped_before(&mut self) {
        self.flags |= 2
    }

    pub fn set_double_faulting(&mut self) {
        self.flags |= 4
    }

    pub fn set_in_fault_trap(&mut self) {
        self.flags |= 8
    }

    pub fn clear_in_fault_trap(&mut self) {
        self.flags &= !8
    }
}
