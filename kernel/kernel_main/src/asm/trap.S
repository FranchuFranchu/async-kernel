.option norvc
.altmacro

.macro store_general i, basereg=t6
	sx	x\i, ((\i)*XLEN)(\basereg)
.endm
.macro load_general i, basereg=t6
	lx	x\i, ((\i)*XLEN)(\basereg)
.endm
.macro store_floating i, basereg=t6
	fsx	f\i, ((32+(\i))*XLEN)(\basereg)
.endm
.macro load_floating i, basereg=t6
	flx	f\i, ((32+(\i))*XLEN)(\basereg)
.endm

.macro store_volatile_registers


	# All registers are volatile here, we need to save them
	# before we do anything.
	csrrw	t6, sscratch, t6
	# csrrw will atomically swap t6 into sscratch and the olx
	# value of sscratch into t6. This is nice because we just
	# switched values and didn't destroy anything -- all atomically!
	# in cpu.rs we have a structure of:
	#  32 gp regs		0
	#  32 fp regs		256
	# We use t6 as the temporary register because it is the very
	# bottom register (x31)
	.set 	i, 0
	.rept	31
		store_general	%i
		.set	i, i+1
	.endr

	# Save the actual t6 register, which we swapped into
	# sscratch
	mv		t5, t6
	csrr	t6, sscratch
	store_general 31, t5

	# Restore the kernel trap frame into sscratch
	csrw	sscratch, t5

	# TODO add fp registers to trap frame
	j 1f

	csrr	t1, sstatus
	srli	t0, t1, 13
	andi	t0, t0, 3
	li		t3, 3
	bne		t0, t3, 1f
	# Save floating point registers
	.set 	i, 0
	.rept	32
		store_floating	%i, t5
		.set	i, i+1
	.endr
	
	1:
.endm

# IN: t6: Trap frame pointer
.macro load_from_trap_frame
	j 1f
	csrr	t1, sstatus
	srli	t0, t1, 13
	andi	t0, t0, 3
	li		t3, 3
	bne		t0, t3, 1f
	.set	i, 0
	.rept	32
		load_floating %i
		.set i, i+1
	.endr
1: # no_f_extension:
	# Restore all GP registers
	.set	i, 1
	.rept	31
		load_general %i
		.set	i, i+1
	.endr
.endm

.macro restore_volatile_registers
	# Now load the trap frame back into t6
	csrr	t6, sscratch

	load_from_trap_frame
	# Since we ran this loop 31 times starting with i = 1,
	# the last one loaded t6 back to its original value.
.endm

.global s_trap_vector
# This must be aligned by 4 since the last two bits
# of the mtvec register do not contribute to the address
# of this vector.
.global critical_code_start
critical_code_start:
.align 4
s_trap_vector:
	store_volatile_registers
	# Now t5 and sscratch have the trap frame pointer
	
	
	# Get ready to go into Rust (trap.rs)
	
	# Load the kernel page table which includes all kernel code including trap.rs
	lx a0, XLEN*38(t5)
	csrw satp, a0
	
	lx 		sp, XLEN*35(t5)
	csrr	a0, sepc
	sx		a0, XLEN*32(t5)
	
	# Usually, We don't want to write into the user's stack or whomever
	# messed with us here.
	# We haven't gotten userspace to work yet, so we can assume that this interrupt was triggered in kernel mode
	
	# la		t0, KERNEL_STACK_END
	# lx		sp, 0(t0)
	
	
	
	call	trap_handler
	
	# When we get here, we've returned from trap_handler, restore registers
	# and return.

	csrr t5, sscratch
	lx a0, XLEN*32(t5)
	csrw	sepc, a0
	
	lx a0, XLEN*37(t5)
	csrw satp, a0
	
	restore_volatile_registers

trap_exit:
	sret
	

.global switch_to_supervisor_frame	
# a0: trap frame
switch_to_supervisor_frame:
	// Load the trap frame
	csrw sscratch, a0
	// When SRET is executed, set PC to the old PC
	lx t0, XLEN*32(a0)
	csrw sepc, t0
	
	lx t0, XLEN*37(a0)
	csrw satp, t0
	
	// If this process got interrupted, it means interrupts were enabled
	li t0, 0x222
	csrw sie, t0
	
	// Set SPP, which will make it so that when SRET is executed, we are in user mode
	csrr t0, sstatus
	li t1, (1 << 8)
	or t0, t1, t0
	csrw sstatus, t0
	
	restore_volatile_registers
	sret

.global switch_to_user_frame	
switch_to_user_frame:
	// Load the trap frame
	csrw sscratch, a0
	// When SRET is executed, set PC to the old PC
	lx t0, XLEN*32(a0)
	csrw sepc, t0
	
	lx t0, XLEN*37(a0)
	csrw satp, t0
	sfence.vma
	
	// If this process got interrupted, it means interrupts were enabled
	li t0, 0x222
	csrw sie, t0
	
	// Clear SPP, which will make it so that when SRET is executed, we are in user mode
	csrr t0, sstatus
	li t1, ~(1 << 8)
	and t0, t1, t0
	csrw sstatus, t0
	
	restore_volatile_registers
	
.global switch_to_user_frame_end
switch_to_user_frame_end:
	sret
	


# Essentially like s_trap_vector, but smode-to-smode
do_syscall_internal:
	
	store_volatile_registers
	# Now t5 and sscratch have the trap frame pointer
	
	# Get ready to go into Rust (trap.rs)
	# csrw	sie, zero
	
	la 		sp, _stack_start
	mv	a0, ra
	sx		a0, XLEN*32(t5)
	li		a1, 0
	csrr	a2, 9
	lx		a0, XLEN*32(t5)
	lx		a3, XLEN*33(t5)
	csrr	a4, sstatus
	csrr	a5, sscratch
	
	# Usually, We don't want to write into the user's stack or whomever
	# messed with us here.
	# We haven't gotten userspace to work yet, so we can assume that this interrupt was triggered in kernel mode
	
	# la		t0, KERNEL_STACK_END
	# lx		sp, 0(t0)
	call	trap_handler
	
	# When we get here, we've returned from m_trap, restore registers
	# and return.
	# m_trap will return the return address via a0.

	mv	ra, a0
	
	restore_volatile_registers
	ret

.global store_to_trap_frame

store_to_trap_frame:
	csrrw a1, sscratch, a1
	
	.set 	i, 0
	.rept	31
		store_general	%i, a0
		.set	i, i+1
	.endr

	
	jal t6, 4
	addi t6, t6, -4
	sx t6, XLEN*32(a0)
	
	
	csrrw a1, sscratch, a1
	ret
	
.global store_to_trap_frame_and_run_function
# a0: Trap frame to save state to
# a1: Function to call
# a2-a7: Other arguments for the function
store_to_trap_frame_and_run_function:
	addi sp, sp, -16
	sx ra, (sp)
	.set 	i, 0
	.rept	32
		store_general	%i, a0
		.set	i, i+1
	.endr
	
	# set pc = .before_return
	la t0, .before_return
	sx t0, XLEN*32(a0)
	
	jalr a1
	
.before_return:
	lx ra, (sp)
	addi sp, sp, 16
	ret


.global critical_code_end	
critical_code_end: