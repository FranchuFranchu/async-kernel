. = ALIGN(4096);
PROVIDE(_stack_end = .);
. = . + 0x10000;
. = ALIGN(4096);
PROVIDE(_stack_start = .);
PROVIDE(_heap_start = .);
PROVIDE(_heap_end = 0x80000000+128M);
PROVIDE(_free_space_start = _heap_end);