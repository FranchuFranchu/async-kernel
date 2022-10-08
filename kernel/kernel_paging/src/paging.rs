//! WIP

use core::fmt::Debug;

use super::{PageLookupError, Table};
use crate::EntryBits;

pub struct PartialMapping {
    // (virt, phys, size)
    diffs: alloc::vec::Vec<(usize, usize, usize)>,
}

impl Debug for PartialMapping {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("PartialMapping {\n");
        for (virt, phys, start) in self.diffs.iter() {
            f.write_fmt(format_args!(
                "    0x{:0>16x} -> 0x{:0>16x} ({:#x})\n",
                virt, phys, start
            ))?;
        }
        f.write_str("}");
        Ok(())
    }
}

impl PartialMapping {
    fn try_get_size(&self) -> Option<usize> {
        let smallest_element = self.diffs.iter().min_by_key(|e| e.0)?;
        let greatest_element = self.diffs.iter().max_by_key(|e| e.0 + e.2)?;
        Some(greatest_element.0 + greatest_element.2 - smallest_element.0)
    }
    pub fn size(&self) -> usize {
        self.try_get_size().unwrap_or(0)
    }
    pub unsafe fn read_iter(&self, phys_to_virt: fn(usize) -> usize) -> impl Iterator<Item = &[u8]> {
        self.diffs.iter().map(move |(virt, phys, size)| core::slice::from_raw_parts(phys_to_virt(*phys) as *const u8, *size))
    }
    pub unsafe fn read_iter_mut(&mut self, phys_to_virt: fn(usize) -> usize) -> impl Iterator<Item = &mut [u8]> {
        self.diffs.iter_mut().map(move |(virt, phys, size)| core::slice::from_raw_parts_mut(phys_to_virt(*phys) as *mut u8, *size))
    }
    pub unsafe fn overwrite_contents_with(&mut self, mut data: impl Iterator<Item = u8>, phys_to_virt: fn(usize) -> usize) {
        for mut buffer in self.read_iter_mut(phys_to_virt) {
            while buffer.len() != 0 {
                if let Some(byte) = data.next() {
                    buffer[0] = byte;
                    buffer = &mut buffer[1..];
                } else {
                    return;
                }
            }
        }
    }
    pub fn trim_size_to(&mut self, max_size: usize) {
        let mut accum_size: usize = 0;
        self.diffs = self.diffs.iter_mut().filter_map(|(v, p, size)| {
            let (v, p, size) = (*v, *p, *size);
            if accum_size >= max_size && size > 0 {
                None
            } else {
                accum_size += size;
                Some((v, p, if accum_size > max_size {
                    // If we've overshot, trim this buffer by the amount we overshot.
                    size - (accum_size - max_size)
                } else {
                    size
                }))
            }
        }).collect();
        assert!(self.size() <= max_size);
    }
}


pub struct GenericPaging<'table, const LEVELS: usize, const PTESIZE: usize>
where
    [(); 4096 / PTESIZE]: Sized,
{
    pub table: &'table mut Table<PTESIZE>,
    pub phys_to_virt: fn(usize) -> usize,
    pub virt_to_phys: fn(usize) -> usize,
}

/// Gets the offset of a virtual page number `i` with ptesize `ptesize`
/// ```rust
/// # fn main() {
/// use kernel_paging::get_vpn_offset;
///
/// assert!(get_vpn_offset(1, 8) == 21);
/// assert!(get_vpn_offset(1, 4) == 22);
/// assert!(get_vpn_offset(0, 8) == 12);
/// assert!(get_vpn_offset(0, 4) == 12);
/// assert!(get_vpn_offset(1, 4) == 22);
/// assert!(get_vpn_offset(2, 4) == 32);
/// # }
/// ```
pub const fn get_vpn_offset(i: usize, ptesize: usize) -> u8 {
    (12 + (i * (12 - ptesize.log2()) as usize)) as u8
}

pub const fn get_entry_power(ptesize: usize) -> usize {
    2usize.pow(12 - ptesize.log2()) - 1
}

pub const fn get_entry_count(ptesize: usize) -> usize {
    4096usize / ptesize
}

/// Gets the virtual page number number of a virtual address as specified in the
/// RISC-V privileged specification
/// ```rust
/// # fn main() {
/// use kernel_paging::get_vpn_number;
///
/// let virt_address: usize = 12 << 21 | 143 << 12 | 2313;
/// assert!(get_vpn_number(virt_address, 0, 8) == 143);
/// assert!(get_vpn_number(virt_address, 1, 8) == 12);
/// let virt_address: usize = 12 << 22 | 143 << 12 | 2313;
/// assert!(get_vpn_number(virt_address, 1, 4) == 12);
/// # }
/// ```
pub const fn get_vpn_number(virtual_address: usize, i: usize, ptesize: usize) -> u16 {
    ((virtual_address >> get_vpn_offset(i, ptesize)) & (get_entry_count(ptesize) - 1)) as u16
}

pub fn set_vpn_number(virtual_address: &mut usize, i: usize, value: u16, ptesize: usize) {
    *virtual_address &= !((get_entry_count(ptesize) - 1) << get_vpn_offset(i, ptesize));
    *virtual_address |= (value as usize) << get_vpn_offset(i, ptesize);
}

/// Gets the size corresponding to a VPN index, or a "i" value in the lookup
/// algorithm ```rust
/// # fn main() {
/// use kernel_paging::get_vpn_size;
///
/// assert!(get_vpn_size(0, 8) == kernel_paging::PAGE_SIZE);
/// assert!(get_vpn_size(1, 8) == kernel_paging::MEGAPAGE_SIZE);
/// # }
/// ```
pub const fn get_vpn_size(i: usize, ptesize: usize) -> usize {
    1 << get_vpn_offset(i, ptesize)
}

fn get_page_offset(virtual_address: usize) -> usize {
    virtual_address & ((1 << 12) - 1)
}

pub trait Paging {
    unsafe fn query(&self, virtual_addr: usize) -> Result<usize, PageLookupError> {
        self.query_permissions(virtual_addr, 0)
    }
    unsafe fn query_permissions(
        &self,
        virtual_addr: usize,
        required_access: u8,
    ) -> Result<usize, PageLookupError>;
    fn map(&mut self, physical_addr: usize, virtual_addr: usize, length: usize, flags: usize);
    unsafe fn query_physical_address(&self, virtual_addr: usize) -> Result<usize, PageLookupError>;
    unsafe fn copy_partial_mapping(&self, virtual_addr: usize, size: usize) -> PartialMapping;
    fn paste_partial_mapping(&mut self, base: usize, partial_mapping: &PartialMapping, flags: usize);
}

impl<'table, const LEVELS: usize, const PTESIZE: usize> Paging
    for GenericPaging<'table, LEVELS, PTESIZE>
where
    [(); 4096 / PTESIZE]: Sized,
{
    unsafe fn query_permissions(
        &self,
        virtual_addr: usize,
        required_access: u8,
    ) -> Result<usize, PageLookupError> {
        // 1. Let a be satp.ppn × PAGESIZE, and let i = LEVELS − 1. (For Sv32,
        // PAGESIZE=212 and LEVELS=2.) The satp register must be active, i.e.,
        // the effective privilege mode must be S-mode or U-mode

        // Here, we're renaming A to table
        let mut table: &Table<PTESIZE> = self.table;
        let mut i = LEVELS - 1;
        loop {
            // 2. Let pte be the value of the PTE at address a+va.vpn[i]×PTESIZE. (For Sv32,
            // PTESIZE=4.) If accessing pte violates a PMA or PMP check, raise
            // an access-fault exception corresponding to the original access
            // type
            let pte = table[get_vpn_number(virtual_addr, i, PTESIZE).into()];
            // 3. If pte.v = 0, or if pte.r = 0 and pte.w = 1, or if any bits or encodings
            // that are reserved for future standard use are set within pte,
            // stop and raise a page-fault exception corresponding
            // to the original access type.
            if pte.value & EntryBits::VALID == 0 {
                return Err(PageLookupError::Invalid);
            } else if pte.value & EntryBits::READ == 0 && pte.value & EntryBits::WRITE == 1 {
                return Err(PageLookupError::WriteOnly);
            } else if PTESIZE > 4 && (pte.value >> 54) & (1 << 7 - 1) != 0 {
                return Err(PageLookupError::ReservedBitSet);
            };
            // 4. Otherwise, the PTE is valid. If pte.r = 1 or pte.x = 1, go to step 5.
            // Otherwise, this PTE is a pointer to the next level of the page
            // table.
            if let Some(this_table) = unsafe {
                pte.try_as_table(self.phys_to_virt)
                    .map(|s| (s as *const Table<PTESIZE>).as_ref().unwrap())
            } {
                // Let i = i − 1. If i < 0, stop and raise a page-fault
                // exception corresponding to the original access type. Otherwise, let a =
                // pte.ppn × PAGESIZE and go to step 2
                if i == 0 {
                    return Err(PageLookupError::PageFault);
                }

                if pte.value & 1 == 0 {
                    return Err(PageLookupError::Invalid);
                }

                // For non-leaf PTEs, the D, A, and U bits are reserved for future standard use.
                // Until their use is defined by a standard extension, they must
                // be cleared by software for forward compatibility.
                if pte.flags() & (EntryBits::DIRTY | EntryBits::ACCESSED | EntryBits::USER) != 0 {
                    return Err(PageLookupError::DAUReservedBitSet);
                }

                i -= 1;
                table = this_table
            } else {
                // 6. If i > 0 and pte.ppn[i − 1 : 0] != 0, this is a misaligned superpage; stop
                // and raise a page-fault exception corresponding to the
                // original access type

                for j in 0..i {
                    if get_vpn_number(pte.address(), j, PTESIZE) != 0 {
                        return Err(PageLookupError::MisalignedSuperpage);
                    }
                }

                // A leaf PTE has been found. Determine if the requested memory access is
                // allowed by the pte.r, pte.w, pte.x, and pte.u bits, given the
                // current privilege mode and the value of the SUM and MXR
                // fields of the mstatus register. If not, stop and raise a page-fault exception
                // corresponding to the original access type.

                let set_bits = required_access as usize & !(pte.flags());
                if set_bits & EntryBits::VALID != 0 {
                    return Err(PageLookupError::NotReadable);
                } else if set_bits & EntryBits::READ != 0 {
                    return Err(PageLookupError::NotReadable);
                } else if set_bits & EntryBits::WRITE != 0 {
                    return Err(PageLookupError::NotWriteable);
                } else if set_bits & EntryBits::EXECUTE != 0 {
                    return Err(PageLookupError::NotExecutable);
                } else if set_bits & EntryBits::USER != 0 {
                    return Err(PageLookupError::NoUserAccess);
                }

                // Step 7 is not done here.

                // 8. The translation is successful. The translated physical address is given as
                // follows:
                // - pa.pgoff = va.pgoff.
                let mut pa = get_page_offset(virtual_addr);

                // - If i > 0, then this is a superpage translation and pa.ppn[i − 1 : 0] =
                //   va.vpn[i − 1 : 0].
                for j in 0..=i {
                    set_vpn_number(
                        &mut pa,
                        j,
                        get_vpn_number(virtual_addr, j, PTESIZE),
                        PTESIZE,
                    );
                }
                // - pa.ppn[LEVELS − 1 : i] = pte.ppn[LEVELS − 1 : i].
                for j in i..=LEVELS - 1 {
                    set_vpn_number(
                        &mut pa,
                        j,
                        get_vpn_number(pte.value << 2, j, PTESIZE),
                        PTESIZE,
                    );
                }
                return Ok(pa);
            }
        }
    }

    fn map(&mut self, physical_addr: usize, virtual_addr: usize, length: usize, flags: usize) {
        fn map_internal<const PTESIZE: usize>(
            level: usize,
            table: &mut Table<PTESIZE>,
            physical_addr: usize,
            virtual_addr: usize,
            length: usize,
            flags: usize,
            current_virt_offset: usize,
            virt_to_phys: fn(usize) -> usize,
            phys_to_virt: fn(usize) -> usize,
        ) where
            [(); 4096 / PTESIZE]: Sized,
        {
            let virt_start = virtual_addr;
            let virt_end = virtual_addr + length - if level == 0 { 0 } else { 1 };
            let vpn_start = get_vpn_number(virt_start, level, PTESIZE) as usize;
            let mut vpn_end = get_vpn_number(virt_end, level, PTESIZE) as usize;
            let offset: usize = physical_addr.wrapping_sub(virtual_addr) >> 2;

            if vpn_end < vpn_start {
                vpn_end = 511;
            }
            for vpn_number in vpn_start..=vpn_end {
                assert!(vpn_number < 512);
                let mut entry = &mut table.entries[vpn_number];
                let current_address =
                    current_virt_offset + vpn_number * get_vpn_size(level, PTESIZE);
                if level != 0 {
                    let start_misaligned = (get_vpn_size(level, PTESIZE) - 1) & virtual_addr != 0;
                    let end_misaligned =
                        (get_vpn_size(level, PTESIZE) - 1) & virtual_addr.wrapping_add(length) != 0;
                    let physical_start_misaligned = (get_vpn_size(level, PTESIZE) - 1)
                        & virtual_addr.wrapping_add(offset << 2)
                        != 0;
                    let physical_end_misaligned = (get_vpn_size(level, PTESIZE) - 1)
                        & virtual_addr.wrapping_add(length).wrapping_add(offset << 2)
                        != 0;
                    if entry.is_leaf()
                        && ((vpn_number == vpn_start
                            && (start_misaligned || physical_start_misaligned))
                            || (vpn_number == vpn_end
                                && (end_misaligned || physical_end_misaligned)))
                    {
                        // Split a megapage
                        unsafe {
                            entry.split::<PTESIZE>(get_vpn_size(level - 1, PTESIZE), virt_to_phys)
                        };
                    }
                    if let Some(table) = unsafe { entry.try_as_table_mut(phys_to_virt) } {
                        map_internal::<PTESIZE>(
                            level - 1,
                            table,
                            physical_addr,
                            virtual_addr,
                            length,
                            flags,
                            current_address,
                            virt_to_phys,
                            phys_to_virt,
                        );
                    } else {
                        entry.value = (current_address >> 2 | flags).wrapping_add(offset)
                            & crate::UNRESERVED_BITS_MASK;
                    }
                } else {
                    entry.value = (current_address >> 2 | flags).wrapping_add(offset)
                        & crate::UNRESERVED_BITS_MASK;
                }
            }
        }
        let virtual_addr = Self::decanonicalize_address(virtual_addr);
        assert!(virtual_addr & 0xfff == 0);
        assert!(physical_addr & 0xfff == 0);
        assert!(length & 0xfff == 0);
        map_internal::<PTESIZE>(
            LEVELS - 1,
            self.table,
            physical_addr,
            virtual_addr,
            length,
            flags,
            0,
            self.virt_to_phys,
            self.phys_to_virt,
        );
    }

    unsafe fn query_physical_address(&self, virtual_addr: usize) -> Result<usize, PageLookupError> {
        self.query(virtual_addr)
    }

    unsafe fn copy_partial_mapping(&self, virtual_addr: usize, size: usize) -> PartialMapping {
        let mut current_segment_size = 0usize;
        let mut virt_start = virtual_addr;
        let mut phys_start = self.query(virtual_addr).unwrap();
        let mut off_here = 0;
        let mut diffs = alloc::vec![];
        
        let capped_size = size.div_ceil(4096) * 4096;
        let extra_size = capped_size - size;
        
        for off in (0..capped_size).step_by(4096) {
            current_segment_size += 4096;

            let expected = phys_start + current_segment_size;
            let got = self.query(virt_start + current_segment_size).unwrap();

            if expected != got {
                diffs.push((virt_start - virtual_addr, phys_start, current_segment_size));
                phys_start = got;
                virt_start = virt_start + current_segment_size;
                current_segment_size = 0;
            }
        }
        if current_segment_size != 0 {
            diffs.push((virt_start - virtual_addr, phys_start, current_segment_size - extra_size));
        }

        PartialMapping { diffs }
    }

    fn paste_partial_mapping(
        &mut self,
        base: usize,
        partial_mapping: &PartialMapping,
        flags: usize,
    ) {
        for (virt, phys, length) in partial_mapping.diffs.iter() {
            self.map(*phys, base + virt, *length, flags);
        }
    }
}

impl<'table, const LEVELS: usize, const PTESIZE: usize> GenericPaging<'table, LEVELS, PTESIZE>
where
    [(); 4096 / PTESIZE]: Sized,
{
    const maximum_noncanon_virtual_address: usize = (1 << get_vpn_offset(LEVELS, PTESIZE)) - 1;

    /// Sign-extends an address
    /// ```rust
    /// # fn main() {
    /// let address = 0x7fffffffff;
    /// let canon = kernel_paging::Sv39::canonicalize_address(address);
    /// assert!(canon == 0xffffffffffffffff);
    /// assert!(kernel_paging::Sv39::canonicalize_address(0x7f12345678) == 0xffffffff12345678)
    /// # }
    /// ```
    pub fn canonicalize_address(address: usize) -> usize {
        let uppermost_significant_bit = get_vpn_offset(LEVELS, PTESIZE) - 1;
        let higher_bits = usize::BITS as usize - uppermost_significant_bit as usize;
        let mask = (1 << higher_bits) - 1;
        let mask = mask << (uppermost_significant_bit + 1);
        if (address >> uppermost_significant_bit) & 1 != 0 {
            address | mask
        } else {
            address & (!mask)
        }
    }

    pub fn decanonicalize_address(address: usize) -> usize {
        let uppermost_significant_bit = get_vpn_offset(LEVELS, PTESIZE) - 1;
        let higher_bits = usize::BITS as usize - uppermost_significant_bit as usize;
        let mask = (1 << higher_bits) - 1;
        let mask = mask << (uppermost_significant_bit + 1);
        address & (!mask)
    }

    pub unsafe fn from_satp(
        satp: usize,
        phys_to_virt: fn(usize) -> usize,
        virt_to_phys: fn(usize) -> usize,
    ) -> Self {
        let table = (phys_to_virt(satp << 12) as *mut Table<PTESIZE>)
            .as_mut()
            .unwrap();
        Self {
            table,
            phys_to_virt,
            virt_to_phys,
        }
    }
}

pub type Sv32<'table> = GenericPaging<'table, 2, 4>;
pub type Sv39<'table> = GenericPaging<'table, 3, 8>;
pub type Sv48<'table> = GenericPaging<'table, 4, 8>;
pub type Sv57<'table> = GenericPaging<'table, 5, 8>;
pub type Sv65<'table> = GenericPaging<'table, 6, 8>; // Doesn't actually exist but, who knows

#[test]
fn test() {
    let mut table = crate::Table::zeroed();
    let mut table2 = Sv32 { table: &mut table };
    table2.map(0, 0, 2usize.pow(31), 7);
    let mut table2 = Sv32 { table: &mut table };
    unsafe {
        assert!(table2.query(0x6000).unwrap() == 0x6000);
    }
    unsafe {
        assert!(table2.query(0x62000).unwrap() == 0x62000);
    }
    unsafe {
        assert!(table2.query(0x23000).unwrap() == 0x23000);
    }
    unsafe {
        assert!(table2.query(0x7fc0_0000).unwrap() == 0x7fc0_0000);
    }
    unsafe {
        assert!(table2.query(0x1fc0_0000).unwrap() == 0x1fc0_0000);
    }
    unsafe {
        assert!(table2.query(0x1020_0000).unwrap() == 0x1020_0000);
    }
    unsafe {
        assert!(Sv32::maximum_noncanon_virtual_address() == 0xffffffff);
    }
}
