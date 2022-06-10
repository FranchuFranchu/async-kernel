.global do_supervisor_syscall
do_supervisor_syscall:
	addi sp, sp, -16
	sx ra, (sp)
	
	csrr t0, sie
	beqz t0, .error_syscall_interrupt_disabled
	
	mv t0, a7
	mv a7, a0
	mv a0, a1
	mv a1, a2
	mv a2, a3
	mv a3, a4
	mv a4, a5
	mv a6, t0
	
	# Set the supervisor software interrupt pending bit (SSIP)
	csrr t0, sip
	ori t0, t0, 1 << 1
	csrw sip, t0
	wfi
	
	

do_supervisor_syscall_end:
	lx ra, (sp)
	addi sp, sp, 16
	ret
	
.error_syscall_interrupt_disabled:
	j syscall_on_interrupt_disabled
	
	lx ra, (sp)
	addi sp, sp, 16
	ret
	
.global do_supervisor_syscall_end