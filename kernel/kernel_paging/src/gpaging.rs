//! WIP

use crate::EntryBits;

use super::{Table, Entry, PageLookupError};

pub struct GenericPaging<'table, const LEVELS: usize, const PTESIZE: usize> 
    where [(); 4096 / PTESIZE]: Sized{
	pub table: &'table mut Table<PTESIZE>
}


/// Gets the offset of a virtual page number `i` with ptesize `ptesize`
/// ```rust
/// # fn main() {
/// use kernel_paging::gpaging::get_vpn_offset;
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
	2usize.pow(12 - ptesize.log2())-1
}

pub const fn get_entry_count(ptesize: usize) -> usize {
	4096usize / ptesize
}


/// Gets the virtual page number number of a virtual address as specified in the RISC-V privileged
/// specification
/// ```rust
/// # fn main() {
/// use kernel_paging::gpaging::get_vpn_number;
///
/// let virt_address: usize = 12 << 21 | 143 << 12 | 2313;
/// assert!(get_vpn_number(virt_address, 0, 8) == 143);
/// assert!(get_vpn_number(virt_address, 1, 8) == 12);
/// let virt_address: usize = 12 << 22 | 143 << 12 | 2313;
/// assert!(get_vpn_number(virt_address, 1, 4) == 12);
/// # }
/// ```
pub const fn get_vpn_number(virtual_address: usize, i: usize, ptesize: usize) -> u16 {
	((virtual_address >> get_vpn_offset(i, ptesize)) & (get_entry_count(ptesize)-1)) as u16
}

pub fn set_vpn_number(virtual_address: &mut usize, i: usize, value: u16, ptesize: usize) {
	*virtual_address &= !((get_entry_count(ptesize)-1) << get_vpn_offset(i, ptesize));
	*virtual_address |= (value as usize) << get_vpn_offset(i, ptesize);
}

/// Gets the size corresponding to a VPN index, or a "i" value in the lookup algorithm
/// ```rust
/// # fn main() {
/// use kernel_paging::gpaging::get_vpn_size;
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

impl<'table, const LEVELS: usize, const PTESIZE: usize> GenericPaging<'table, LEVELS, PTESIZE> 
    where [(); 4096 / PTESIZE]: Sized {
    pub unsafe fn query(&self, virtual_addr: usize) -> Result<usize, PageLookupError> {
    	const PAGESIZE: usize = 4096;
    	// 1. Let a be satp.ppn × PAGESIZE, and let i = LEVELS − 1. (For Sv32, PAGESIZE=212 and
		// LEVELS=2.) The satp register must be active, i.e., the effective privilege mode must be
		// S-mode or U-mode
		
		// Here, we're renaming A to table
		let mut table: &Table<PTESIZE> = self.table;
    	let mut i = LEVELS - 1;
    	loop {
	    	// 2. Let pte be the value of the PTE at address a+va.vpn[i]×PTESIZE. (For Sv32, PTESIZE=4.)
			// If accessing pte violates a PMA or PMP check, raise an access-fault exception corresponding
			// to the original access type
	    	let pte = table[get_vpn_number(virtual_addr, i, PTESIZE).into()];
	    	// 3. If pte.v = 0, or if pte.r = 0 and pte.w = 1, or if any bits or encodings that are reserved for
			// future standard use are set within pte, stop and raise a page-fault exception corresponding
			// to the original access type.
	    	if pte.value & EntryBits::VALID == 0 {
	    		return Err(PageLookupError::Invalid)
	    	} else if pte.value & EntryBits::READ == 0 && pte.value & EntryBits::WRITE == 1 {
	    		return Err(PageLookupError::WriteOnly)
	    	} else if PTESIZE > 4 && (pte.value >> 54) & (1 << 7 - 1) != 0 {
	    		return Err(PageLookupError::ReservedBitSet)
	    	};
	    	// 4. Otherwise, the PTE is valid. If pte.r = 1 or pte.x = 1, go to step 5. Otherwise, this PTE is a
			// pointer to the next level of the page table. 
    		if let Some(this_table) = unsafe { pte.try_as_table().map(|s| (s as *const Table<PTESIZE>).as_ref().unwrap()) } {
    			// Let i = i − 1. If i < 0, stop and raise a page-fault
				// exception corresponding to the original access type. Otherwise, let a = pte.ppn × PAGESIZE
				// and go to step 2
    			if i == 0 {
    				return Err(PageLookupError::PageFault)
    			}
    			i -= 1;
    			table = this_table
    		} else {
    			// Steps 5 to 7 are not done here.
    			
    			// 8. The translation is successful. The translated physical address is given as follows:
				// - pa.pgoff = va.pgoff.
				let mut pa = get_page_offset(virtual_addr);
				
				// - If i > 0, then this is a superpage translation and pa.ppn[i − 1 : 0] = va.vpn[i − 1 : 0].
				for j in 0..=i {
					set_vpn_number(&mut pa, j, get_vpn_number(virtual_addr, j, PTESIZE), PTESIZE);
				}
				// - pa.ppn[LEVELS − 1 : i] = pte.ppn[LEVELS − 1 : i].
				for j in i..=LEVELS-1 {
					set_vpn_number(&mut pa, j, get_vpn_number(pte.value << 2, j, PTESIZE), PTESIZE);
				}
				return Ok(pa);
    		}
    	}
    }
    pub fn map(&mut self, physical_addr: usize, virtual_addr: usize, length: usize, flags: usize) {
    	fn map_internal<const PTESIZE: usize>(level: usize, table: &mut Table<PTESIZE>, physical_addr: usize, virtual_addr: usize, length: usize, flags: usize, current_virt_offset: usize) 
		    where [(); 4096 / PTESIZE]: Sized{
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
	    		let current_address = current_virt_offset + vpn_number * get_vpn_size(level, PTESIZE);
	    		if level != 0 { 
		    		if (
		    			(((vpn_number == vpn_start) && ((get_vpn_size(level, PTESIZE)) - 1) & virtual_addr != 0)) ||
		    			(((vpn_number == vpn_end) && ((get_vpn_size(level, PTESIZE)) - 1) & (virtual_addr.wrapping_add(length)) != 0)))
		    			&& entry.is_leaf() {
		    			// Split a megapage
		    			unsafe { entry.split::<PTESIZE>(get_vpn_size(level-1, PTESIZE)) };
		    		}  
		    		if let Some(table) = unsafe { entry.try_as_table_mut() } {
                        map_internal::<PTESIZE>(level - 1, table, physical_addr, virtual_addr, length, flags, current_address);
                    } else {
                        entry.value = (current_address >> 2 | flags).wrapping_add(offset) & crate::UNRESERVED_BITS_MASK;
                    }
		    	} else {
		    		entry.value = (current_address >> 2 | flags).wrapping_add(offset) & crate::UNRESERVED_BITS_MASK;
		    	}
	    	}
	    }
    	map_internal::<PTESIZE>(LEVELS-1, self.table, physical_addr, virtual_addr, length, flags, 0);
    }
    pub unsafe fn query_physical_address(&self, virtual_addr: usize) -> Result<usize, PageLookupError> {
        self.query(virtual_addr)
    }
    pub const fn maximum_virtual_address() -> usize {
        (1 << get_vpn_offset(LEVELS, PTESIZE)) - 1
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
	let mut table2 = Sv32{ table: &mut table };
	table2.map(0, 0, 2usize.pow(31), 7);
	let mut table2 = Sv32{ table: &mut table };
	unsafe { assert!(table2.query(0x6000).unwrap() == 0x6000); }
	unsafe { assert!(table2.query(0x62000).unwrap() == 0x62000); }
	unsafe { assert!(table2.query(0x23000).unwrap() == 0x23000); }
	unsafe { assert!(table2.query(0x7fc0_0000).unwrap() == 0x7fc0_0000); }
	unsafe { assert!(table2.query(0x1fc0_0000).unwrap() == 0x1fc0_0000); }
	unsafe { assert!(table2.query(0x1020_0000).unwrap() == 0x1020_0000); }
	std::println!("{:x}", Sv32::maximum_virtual_address());
	std::println!("{:x}", get_vpn_offset(2, 4));
	unsafe { assert!(Sv32::maximum_virtual_address() == 0xffffffff); }
}
