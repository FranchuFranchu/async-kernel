.section .text.init
.global boot
.extern _stack_start
boot:
	la sp, _stack_start
	call pre_main
	
.section .text
.global hart_entry_point
hart_entry_point:
	mv sp, a1
	ld t0, 0(sp)
	csrw satp, t0
	call hart_entry

