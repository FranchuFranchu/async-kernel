ENTRY (boot)

PHDRS {
	all PT_LOAD;
};

SECTIONS {
  	HIGH_OFFSET = 0xffffffff80000000;
	. = 0xffffffff80000000;
	
	INCLUDE "common_sections.ld"
	INCLUDE "virt.ld"
}