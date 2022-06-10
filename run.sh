#!/bin/bash

# .cargo/config sets our argv
# $0 = this file
# $1 = bits
# $2 = architecture

export ARCH=$2
export BITS=$1


if [ "$GDB" == "yes" ]; then
	lxterminal -e 'riscv'$BITS'-elf-gdb target/'$ARCH'-unknown-none-elf/debug/rust-0bsd-riscv-kernel\
	-ex "target remote tcp4:0:1234"\
	-ex "break rust_0bsd_riscv_kernel::panic"\
	-ex "alias print_hartids = p [\$mhartid, rust_0bsd_riscv_kernel::cpu::load_hartid()]"\
	-ex "alias phids = print_hartids"\
	-ex "set history save on"\
	' &
	export QEMUOPTS="-S -s $QEMUOPTS"
fi
export QEMUOPT_D="guest_errors,unimp"
if [ "$INT" == "yes" ]; then
	export QEMUOPT_D="int,$QEMUOPT_D"
fi

if [ "$GFX" == "yes" ]; then 
	export QEMUOPTS="-device virtio-gpu-device $QEMUOPTS"
else
	export QEMUOPTS="-nographic $QEMUOPTS"
fi

qemu-system-riscv$BITS $QEMUOPTS \
	-machine virt \
	-cpu rv$BITS \
	-chardev stdio,id=console,mux=on \
	-serial chardev:console \
	-monitor chardev:console \
	-d $QEMUOPT_D \
	-blockdev driver=file,filename=`dirname $0`/drive.img,node-name=hda \
	-device virtio-blk-device,drive=hda \
	-smp 1 \
	-m 128M \
	-device ne2k_pci \
	-kernel $3
