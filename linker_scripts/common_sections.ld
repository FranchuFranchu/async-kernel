.text.init ALIGN(4096) : {
	*(.text.init)
}: all

.text ALIGN(4096) : AT ( ADDR(.text) - HIGH_OFFSET ) {
	*(.text)
	*(.text.*)
}: all
.rodata ALIGN(4096) : AT ( ADDR(.bss) - HIGH_OFFSET ) {
	*(.rodata)
	*(.rodata.*)
}: all
.data ALIGN(4096) : AT ( ADDR(.data) - HIGH_OFFSET ) {
	*(.data)
	*(.data.*)
	*(.sdata)
	*(.sdata.*)
}: all
.bss ALIGN(4096) : AT ( ADDR(.bss) - HIGH_OFFSET ) {
	*(.bss)
	*(.bss.*)
}: all
.sbss ALIGN(4096) : AT ( ADDR(.sbss) - HIGH_OFFSET ) {
	*(.sbss)
	*(.sbss.*)
}: all

.eh_frame : {
	*(.eh_frame)
}: all