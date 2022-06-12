#!/bin/bash

# $0 = this file
# $1 = bits
# $2 = architecture

export ARCH=$2
export BITS=$1

LIB_BASE=kernel/target/$ARCH/debug

riscv$BITS-elf-objcopy -O binary $LIB_BASE/kernel_main kernel_payload.bin