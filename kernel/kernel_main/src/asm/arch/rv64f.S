.set XLEN, 8   # Register size (in bytes)
.global XLEN

# To make this file compatible with both rv32 and rv64
# Store XLEN bytes
.macro lx a, b
ld \a, \b
.endm
.macro sx a, b
sd \a, \b
.endm

.macro flx a, b
fld \a, \b
.endm
.macro fsx a, b
fsd \a, \b
.endm
