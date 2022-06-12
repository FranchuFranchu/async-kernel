# This is reached when the M-mode software (bootloader) has loaded us here
.section .text.init
.extern _stack_start
.global boot
boot:
	li sp, 0x84000000
	la t0, main
	jalr t0
	
.global ret_
ret_:
	ret
	
.section .text
