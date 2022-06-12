ENTRY ( boot )
 
PHDRS {
	all PT_LOAD;
};

SECTIONS {
	. = 0x80200000;
  	HIGH_OFFSET = 0;
  	
  	
	.text ALIGN(4096) : AT ( ADDR(.text) - HIGH_OFFSET ) {
		*(.text.init)
		*(.text)
		*(.text.*)
	} : all
	.rodata ALIGN(4096) : AT ( ADDR(.bss) - HIGH_OFFSET ) {
		*(.rodata)
		*(.rodata.*)
	} : all
	.data ALIGN(4096) : AT ( ADDR(.data) - HIGH_OFFSET ) {
		*(.data)
		*(.data.*)
		*(.sdata)
		*(.sdata.*)
	} : all
	.bss ALIGN(4096) : AT ( ADDR(.bss) - HIGH_OFFSET ) {
		*(.bss)
		*(.bss.*)
	} : all
	.sbss ALIGN(4096) : AT ( ADDR(.sbss) - HIGH_OFFSET ) {
		*(.sbss)
		*(.sbss.*)
	} : all
  	
  	. = 0x81000000;
	.payload : AT ( ADDR(.payload) ){
		*(.payload)
		*(.payload.*)
	} : all
  	INCLUDE "virt.ld"
}

