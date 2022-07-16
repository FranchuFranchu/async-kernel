_execute_start = .;
.text.init ALIGN(4096) : {
	*(.text.init)
}: all

.text : AT ( ADDR(.text) - HIGH_OFFSET ) {
	*(.text)
	*(.text.*)
}: all
. = ALIGN(4096);
_execute_end = .;
_readonly_start = .;
.rodata : AT ( ADDR(.bss) - HIGH_OFFSET ) {
	*(.rodata)
	*(.rodata.*)
}: all
. = ALIGN(4096);
_readonly_end = .;
_readwrite_start = .;
.data : AT ( ADDR(.data) - HIGH_OFFSET ) {
	*(.data)
	*(.data.*)
	*(.sdata)
	*(.sdata.*)
}: all
.bss : AT ( ADDR(.bss) - HIGH_OFFSET ) {
	*(.bss)
	*(.bss.*)
}: all
.sbss : AT ( ADDR(.sbss) - HIGH_OFFSET ) {
	*(.sbss)
	*(.sbss.*)
}: all
. = ALIGN(4096);
_readwrite_end = .;
_stack_heap_start = .;

.eh_frame : {
	*(.eh_frame)
}: all