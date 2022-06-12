.section .text.init
.global boot
.extern _stack_start
boot:
	la sp, _stack_start
	call pre_main
.section .text
