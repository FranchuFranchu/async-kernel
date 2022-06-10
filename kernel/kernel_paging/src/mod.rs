//! Abstractions over supervisor-mode paging

use core::{
    fmt::Debug,
    ops::{Index, IndexMut},
};

pub mod sv32;
#[cfg(target_arch = "riscv64")]
pub mod sv39;
#[cfg(target_arch = "riscv64")]
pub mod sv48;

pub mod gpaging;

pub mod EntryBits {
    // The V bit indicates whether the PTE is valid; if it is 0, all other bits in the PTE are don’t-cares and may be used freely by software.
    pub const VALID: usize = 1 << 0;
    // The permission bits, R, W, and X, indicate whether the page is readable, writable, and executable, respectively.When all three are zero, the PTE is a pointer to the next level of the page table; otherwise, it isa leaf PTE. Writable pages must also be marked readable; the contrary combinations are reservedfor future use.  Table 4.4 summarizes the encoding of the permission bits.
    // XWR Meaning
    // 000 Pointer to next level of page table
    // 001 Read-only page
    // 010 Reserved for future use
    // 011 Read-write page
    // 100 Execute-only page
    // 101 Read-execute page
    // 110 Reserved for future use
    // 111 Read-write-execute page
    pub const READ: usize = 1 << 1;
    pub const WRITE: usize = 1 << 2;
    pub const EXECUTE: usize = 1 << 3;
    // The U bit indicates whether the page is accessible to user mode.  U-mode software may only accessthe page when U=1.  If the SUM bit in thesstatusregister is set, supervisor mode software mayalso access pages with U=1.
    pub const USER: usize = 1 << 4;
    // The G bit designates a global mapping.  Global mappings are those that exist in all address spaces.For non-leaf PTEs, the global setting implies that all mappings in the subsequent levels of the pagetable are global.  Note that failing to mark a global mapping as global merely reduces performance,whereas  marking  a  non-global  mapping  as  global  is  a  software  bug  that,  after  switching  to  anaddress space with a different non-global mapping for that address range, can unpredictably resultin either mapping being used.
    pub const GLOBAL: usize = 1 << 5;
    // Each leaf PTE contains an accessed (A) and dirty (D) bit.  The A bit indicates the virtual page hasbeen read, written, or fetched from since the last time the A bit was cleared.  The D bit indicatesthe virtual page has been written since the last time the D bit was cleared.
    pub const ACCESSED: usize = 1 << 6;
    pub const DIRTY: usize = 1 << 7;

    pub const ADDRESS_MASK: usize = usize::MAX ^ ((1 << 8) - 1);
    pub const RWX: usize = READ | WRITE | EXECUTE;

    pub const CODE_SUPERVISOR: usize = 1 << 1 | 1 << 3 | 1;
    pub const DATA_SUPERVISOR: usize = 1 << 1 | 1 << 2 | 1;
}

#[derive(Debug)]
pub enum PageLookupError {
    PageFault,
    AccessFault,
    /// bits 54 or more are set
    ReservedBitSet,
    /// if pte.v = 0, 
    Invalid,
    /// if pte.r = 0 and pte.w = 1,
    WriteOnly,
    
}

#[derive(Default, Copy, Clone)]
pub struct Entry {
    pub value: usize,
}

impl Entry {
    pub const fn zeroed() -> Self {
        Entry { value: 0 }
    }
}

impl Entry {
    /// # Safety
    /// The entry's value must be a valid physical address pointer
    pub unsafe fn as_table_mut(&mut self) -> &mut Table {
        assert!(self.value & 1 != 0);
        (self.address() as *mut Table)
            .as_mut()
            .unwrap()
    }
    /// # Safety
    /// The entry's value must be a valid physical address pointer
    pub unsafe fn as_table(&self) -> &Table {
        assert!(self.value & 1 != 0);
        (self.address() as *mut Table)
            .as_ref()
            .unwrap()
    }

    pub unsafe fn try_as_table_mut(&mut self) -> Option<&mut Table> {
        if self.is_leaf() {
            None
        } else {
            Some(self.as_table_mut())
        }
    }
    pub unsafe fn try_as_table(&self) -> Option<&Table> {
        if self.is_leaf() {
            None
        } else {
            Some(self.as_table())
        }
    }
    
    pub fn ppn_index(&self, index: u8) -> u8 {
        ((self.value >> 10) >> index * 8) as u8 & u8::MAX
    }

    pub fn is_leaf(&self) -> bool {
        (self.value & (EntryBits::READ | EntryBits::EXECUTE)) != 0
            || (self.value & EntryBits::VALID == 0)
            || (self.value & EntryBits::ADDRESS_MASK) == 0
    }
    /// This takes a leaf entry and turns it into a reference to a page table with the same effect.
    /// Increment should be one of the PAGE_SIZE, MEGAPAGE_SIZE, GIGAPAGE_SIZE, etc constants
    /// If this entry is a megapage, for example, the increment should be PAGE_SIZE

    pub unsafe fn split(&mut self, increment: usize) {
        let mut table = Box::new(Table::zeroed());
        let mut current_address = self.value & EntryBits::ADDRESS_MASK;

        let flags = self.value & !(EntryBits::ADDRESS_MASK);

        for entry in table.entries.iter_mut() {
            entry.value = flags | current_address;
            current_address += increment >> 2;
        }
        self.value = 1 | ((&*table as *const Table as usize) >> 2);
        Box::leak(table);

        debug_assert!(!self.is_leaf());
        debug_assert!(self.value & 1 != 0);
        debug_assert!(self.value & EntryBits::RWX == 0);
    }
    
    pub fn address(&self) -> usize {
        (self.value & EntryBits::ADDRESS_MASK) << 2
    }
}

impl Debug for Entry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use core::fmt::Write;

        use EntryBits::*;
        f.write_str("<Entry: ")?;
        if self.value & VALID == 0 {
            f.write_fmt(format_args!("Invalid entry: {:x}", self.value))?;
        } else if self.value & RWX == 0 {
            f.write_fmt(format_args!(
                "Table to: {:x}",
                (self.value & ADDRESS_MASK) << 2
            ))?;
        } else {
            if self.value & READ != 0 {
                f.write_char('R')?;
            }
            if self.value & WRITE != 0 {
                f.write_char('W')?;
            }
            if self.value & EXECUTE != 0 {
                f.write_char('X')?;
            }
            if self.value & USER != 0 {
                f.write_char('U')?;
            }
            f.write_char(' ')?;
            f.write_fmt(format_args!("{:x}", (self.value & ADDRESS_MASK) << 2))?;
        }
        f.write_char('>')?;
        Ok(())
    }
}

#[repr(C)]
#[repr(align(4096))]
#[derive(Debug)]
pub struct Table {
    pub entries: [Entry; 512],
}

impl Table {
    pub const fn zeroed() -> Self {
        Table {
            entries: [Entry { value: 0 }; 512],
        }
    }
}

impl Index<usize> for Table {
    type Output = Entry;
    fn index(&self, idx: usize) -> &Entry {
        &self.entries[idx]
    }
}

impl IndexMut<usize> for Table {
    fn index_mut(&mut self, idx: usize) -> &mut Entry {
        &mut self.entries[idx]
    }
}

pub trait Paging {
    fn map(&mut self, physical_addr: usize, virtual_addr: usize, length: usize, flags: usize);
    unsafe fn query(&self, virtual_addr: usize) -> Result<(Entry, usize), PageLookupError>;
    unsafe fn query_physical_address(&self, virtual_addr: usize) -> Result<usize, PageLookupError> {
        self.query(virtual_addr).map(|(entry, offset)| entry.address() + offset)
    }
    fn identity_map(&mut self);
}

pub unsafe fn enable(_root_table_physical: usize) {}

use core::ffi::c_void;

extern "C" {
    static critical_code_start: c_void;
    static critical_code_end: c_void;
}

/// Map the trap and switch to user/supervisor frame functions, which are the ones that change SATP to change contexts
pub fn map_critical_kernel_address_space(table: &mut impl Paging, trap_frame: usize) {
    let start = unsafe { &critical_code_start as *const c_void as usize }.div_floor(4096) * 4096;

    let end = unsafe { &critical_code_end as *const c_void as usize }.div_ceil(4096) * 4096;
    table.map(
        start,
        start,
        end - start,
        EntryBits::VALID | EntryBits::READ | EntryBits::EXECUTE,
    );
    table.map(
        trap_frame,
        trap_frame,
        4096,
        EntryBits::VALID | EntryBits::READ | EntryBits::WRITE,
    );
}

use alloc::boxed::Box;

use kernel_cpu::csr::{SATP_BARE, SATP_SV39};

pub trait PagingDebug: Paging + Debug {}
pub fn get_active_root_table(satp: usize) -> Option<Box<dyn PagingDebug>> {
    let paging_type = (satp >> 60) << 60;
    let table_addr = (satp & ((1 << 60) - 1)) << 12;
    match paging_type {
        SATP_BARE => None,
        SATP_SV39 => unsafe {
            Some(Box::new(sv39::RootTable(
                (table_addr as *mut Table).as_mut().unwrap(),
            )))
        },
        _ => todo!(),
    }
}

pub const ENTRY_COUNT: usize = 512;
pub const PAGE_ALIGN: usize = 4096;
pub const PAGE_SIZE: usize = PAGE_ALIGN;
pub const MEGAPAGE_SIZE: usize = PAGE_ALIGN * ENTRY_COUNT;
#[cfg(target_arch = "riscv64")]
pub const GIGAPAGE_SIZE: usize = PAGE_ALIGN * ENTRY_COUNT * ENTRY_COUNT;
#[cfg(target_arch = "riscv64")]
pub const TERAPAGE_SIZE: usize = PAGE_ALIGN * ENTRY_COUNT * ENTRY_COUNT * ENTRY_COUNT;
