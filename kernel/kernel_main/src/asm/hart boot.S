# Entry point for harts other than the boot hart
# a1 = opaque = kernel-allocated stack for this hart
.global new_hart
new_hart:
	mv sp, a1
	j hart_entry