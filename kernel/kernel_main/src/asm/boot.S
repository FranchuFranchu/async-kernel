# This is reached when the M-mode software (bootloader) has loaded us here
.section .text.init
.global boot
boot:
	la sp, _stack_start
	j main
	
.global ret_
ret_:
	ret
	
.section .text