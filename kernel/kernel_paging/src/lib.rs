#![cfg_attr(not(test), no_std)]
#![feature(
    int_roundings,
    generic_const_exprs,
    int_log,
    new_uninit,
    maybe_uninit_as_bytes
)]

mod paging;
pub use paging::*;

// Abstractions over supervisor-mode paging
extern crate alloc;
#[cfg(test)]
#[macro_use]
extern crate std;

#[cfg(not(test))]
#[macro_use]
pub mod debug;
use core::{
    fmt::Debug,
    ops::{Index, IndexMut},
};

pub mod EntryBits {
    // The V bit indicates whether the PTE is valid; if it is 0, all other bits in
    // the PTE are don’t-cares and may be used freely by software.
    pub const VALID: usize = 1 << 0;
    // The permission bits, R, W, and X, indicate whether the page is readable,
    // writable, and executable, respectively.When all three are zero, the PTE is a
    // pointer to the next level of the page table; otherwise, it isa leaf PTE.
    // Writable pages must also be marked readable; the contrary combinations are
    // reservedfor future use.  Table 4.4 summarizes the encoding of the permission
    // bits. XWR Meaning
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
    // The U bit indicates whether the page is accessible to user mode.  U-mode
    // software may only accessthe page when U=1.  If the SUM bit in
    // thesstatusregister is set, supervisor mode software mayalso access pages with
    // U=1.
    pub const USER: usize = 1 << 4;
    // The G bit designates a global mapping.  Global mappings are those that exist
    // in all address spaces.For non-leaf PTEs, the global setting implies that all
    // mappings in the subsequent levels of the pagetable are global.  Note that
    // failing to mark a global mapping as global merely reduces performance,whereas
    // marking  a  non-global  mapping  as  global  is  a  software  bug  that,
    // after  switching  to  anaddress space with a different non-global mapping for
    // that address range, can unpredictably resultin either mapping being used.
    pub const GLOBAL: usize = 1 << 5;
    // Each leaf PTE contains an accessed (A) and dirty (D) bit.  The A bit
    // indicates the virtual page hasbeen read, written, or fetched from since the
    // last time the A bit was cleared.  The D bit indicatesthe virtual page has
    // been written since the last time the D bit was cleared.
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
    /// if pte.r = 0 and read access
    NotReadable,
    /// if pte.w = 0 and write access
    NotWriteable,
    /// if pte.x = 0 and execute access
    NotExecutable,
    /// if pte.u = 0 and user access
    NoUserAccess,
    /// if pte.rwx == 0 (non-leaf PTE) and pte.dau != 0
    DAUReservedBitSet,
    MisalignedSuperpage,
}

#[derive(Default, Copy, Clone, Eq, PartialEq)]
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
    pub unsafe fn as_table_mut<const PTESIZE: usize>(
        &mut self,
        phys_to_virt: fn(usize) -> usize,
    ) -> &mut Table<PTESIZE>
    where
        [(); 4096 / PTESIZE]: Sized,
    {
        assert!(self.value & 1 != 0);
        (phys_to_virt(self.address()) as *mut Table<PTESIZE>)
            .as_mut()
            .unwrap()
    }

    /// # Safety
    /// The entry's value must be a valid physical address pointer
    pub unsafe fn as_table<const PTESIZE: usize>(
        &self,
        phys_to_virt: fn(usize) -> usize,
    ) -> &Table<PTESIZE>
    where
        [(); 4096 / PTESIZE]: Sized,
    {
        assert!(self.value & 1 != 0);
        (phys_to_virt(self.address()) as *mut Table<PTESIZE>)
            .as_ref()
            .unwrap()
    }

    pub unsafe fn try_as_table_mut<const PTESIZE: usize>(
        &mut self,
        phys_to_virt: fn(usize) -> usize,
    ) -> Option<&mut Table<PTESIZE>>
    where
        [(); 4096 / PTESIZE]: Sized,
    {
        if self.is_leaf() {
            None
        } else {
            Some(self.as_table_mut(phys_to_virt))
        }
    }

    pub unsafe fn try_as_table<const PTESIZE: usize>(
        &self,
        phys_to_virt: fn(usize) -> usize,
    ) -> Option<&Table<PTESIZE>>
    where
        [(); 4096 / PTESIZE]: Sized,
    {
        if self.is_leaf() {
            None
        } else {
            Some(self.as_table(phys_to_virt))
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

    /// This takes a leaf entry and turns it into a reference to a page table
    /// with the same effect. Increment should be one of the PAGE_SIZE,
    /// MEGAPAGE_SIZE, GIGAPAGE_SIZE, etc constants If this entry is a
    /// megapage, for example, the increment should be PAGE_SIZE

    pub unsafe fn split<const PTESIZE: usize>(
        &mut self,
        increment: usize,
        virt_to_phys: fn(usize) -> usize,
    ) where
        [(); 4096 / PTESIZE]: Sized,
    {
        let mut table = Table::<PTESIZE>::boxed_zeroed();
        let mut current_address = self.value & EntryBits::ADDRESS_MASK;

        let flags = self.value & !(EntryBits::ADDRESS_MASK);

        for entry in table.entries.iter_mut() {
            entry.value = flags | current_address;
            current_address += increment >> 2;
        }
        self.value = 1 | (virt_to_phys(&*table as *const Table<PTESIZE> as usize) >> 2);
        
        // TODO somehow store this somewhere instead of leaking it.
        Box::leak(table);

        debug_assert!(!self.is_leaf());
        debug_assert!(self.value & 1 != 0);
        debug_assert!(self.value & EntryBits::RWX == 0);
    }

    pub fn address(&self) -> usize {
        (self.value & EntryBits::ADDRESS_MASK) << 2
    }

    pub fn flags(&self) -> usize {
        self.value & !EntryBits::ADDRESS_MASK
    }
    
    pub fn from_phys_addr_and_flags(phys: usize, flags: usize) -> Self {
        Entry { value: phys >> 2 | flags }
    }
    
    pub unsafe fn deep_clone<const PTESIZE: usize>(&self, phys_to_virt: fn(usize) -> usize, virt_to_phys: fn(usize) -> usize) -> Self  where [(); 4096 / PTESIZE]: Sized {
        if let Some(table) = self.try_as_table::<PTESIZE>(phys_to_virt) {
            let table = Box::new(table.clone_with(phys_to_virt, virt_to_phys));
            let addr = Box::into_raw(table) as usize;
            let addr = virt_to_phys(addr);
            Self::from_phys_addr_and_flags(addr, 1)
        } else {
            *self
        }
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
            if self.value & ACCESSED != 0 {
                f.write_char('A')?;
            }
            if self.value & DIRTY != 0 {
                f.write_char('D')?;
            }
            f.write_char(' ')?;
            f.write_fmt(format_args!("{:x}", (self.value & ADDRESS_MASK) << 2))?;
        }
        f.write_char('>')?;
        Ok(())
    }
}

#[cfg(target_arch = "riscv32")]
pub const ARCH_PTESIZE: usize = 4;
#[cfg(target_arch = "riscv64")]
pub const ARCH_PTESIZE: usize = 8;

pub type ArchTable = Table<{ ARCH_PTESIZE }>;

#[repr(C)]
#[repr(align(4096))]
#[derive(Debug, PartialEq, Eq)]
pub struct Table<const PTESIZE: usize>
where
    [(); 4096 / PTESIZE]: Sized,
{
    pub entries: [Entry; 4096 / PTESIZE],
}

impl<const PTESIZE: usize> Table<PTESIZE>
where
    [(); 4096 / PTESIZE]: Sized,
{
    pub const fn zeroed() -> Self {
        Table {
            entries: [Entry { value: 0 }; 4096 / PTESIZE],
        }
    }

    pub fn boxed_zeroed() -> Box<Self> {
        let mut table = Box::new_uninit();
        table.as_bytes_mut().into_iter().for_each(|s| {
            s.write(0);
        });
        unsafe { table.assume_init() }
    }
    
    pub fn from_satp(
        satp: usize,
        phys_to_virt: fn(usize) -> usize,
    ) -> *const Table<PTESIZE> {
        phys_to_virt(satp << 12) as *const Table<PTESIZE>
    }
    pub fn to_satp_base_addr(
        &self,
        virt_to_phys: fn(usize) -> usize,
    ) -> usize {
        virt_to_phys(self.entries.as_ptr() as usize) >> 12
    }
    
    pub unsafe fn clone_with(
        &self,
        phys_to_virt: fn(usize) -> usize,
        virt_to_phys: fn(usize) -> usize,
    ) -> Self {
        // Dumb shallow clone
        let mut new_entries = self.entries.clone();
        
        for i in new_entries.iter_mut() {
            *i = i.deep_clone::<PTESIZE>(phys_to_virt, virt_to_phys);
        }
        
        Table { entries: new_entries }
    }
}

impl<const PTESIZE: usize> Index<usize> for Table<PTESIZE>
where
    [(); 4096 / PTESIZE]: Sized,
{
    type Output = Entry;

    fn index(&self, idx: usize) -> &Entry {
        &self.entries[idx]
    }
}

impl<const PTESIZE: usize> IndexMut<usize> for Table<PTESIZE>
where
    [(); 4096 / PTESIZE]: Sized,
{
    fn index_mut(&mut self, idx: usize) -> &mut Entry {
        &mut self.entries[idx]
    }
}

use alloc::boxed::Box;

pub const ENTRY_COUNT: usize = 512;
pub const PAGE_ALIGN: usize = 4096;
pub const PAGE_SIZE: usize = PAGE_ALIGN;
pub const MEGAPAGE_SIZE: usize = PAGE_ALIGN * ENTRY_COUNT;
#[cfg(target_arch = "riscv64")]
pub const GIGAPAGE_SIZE: usize = PAGE_ALIGN * ENTRY_COUNT * ENTRY_COUNT;
#[cfg(target_arch = "riscv64")]
pub const TERAPAGE_SIZE: usize = PAGE_ALIGN * ENTRY_COUNT * ENTRY_COUNT * ENTRY_COUNT;
pub const UNRESERVED_BITS_MASK: usize = 2usize.pow(54) - 1;
