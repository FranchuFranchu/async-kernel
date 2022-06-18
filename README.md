# async-kernel

Mostly 0BSD-licensed `RV64GC` Rust kernel for Qemu's `virt` machine

Files not made by me (and possibly not 0BSD-licensed) are `kernel/kernel_main/asm/trap.S`.

Based off my earlier project `rust-0bsd-riscv-kernel`.

I learnt some things from this earlier project, so this kernel is designed differently.

## Code flow

### Compilation

1. `kernel/kernel_main` gets compiled with `usize::MAX - 0x8000_0000` as its base address. This means that all
kernel routines will think that they are located in the last `0x8000_0000` bytes of memory.
2. The code in resulting ELF file gets dumped using `objcopy` into `kernel_payload.bin`. The first byte is at `usize::MAX - 0x8000_0000`. This is done in `link.sh`
3. `kernel/kernel_bootloader` is compiled with its base address at `0x8020_0000`. It uses `include_bytes!` to include `kernel_payload.bin` in its final binary.
4. qemu is launched, with the binary resulting from the compilation of `kernel_bootloader` as the kernel image.

### Code flow

1. OpenSBI runs before the kernel. This is provided by the platform. It loads the kernel image and puts it at physical address `0x8020_0000`.
2. OpenSBI jumps to `0x8020_0000`. The code there is at `kernel/kernel_bootloader/boot.S`.
3. The `pre_main` function is run. It sets up a small heap based on hardcoded memory addresses. It creates a page table where physical memory is mapped to the higher half of the virtual address space.
4. In addition, `pre_main` maps the kernel image in the same page table to `usize::MAX - 0x8000_0000`.
5. `pre_main` jumps to `usize::MAX - 0x8000_0000`. This is `kernel_main::boot`, a naked function this time.
6. Rest of the kernel gets called by `kernel_main::boot`. TODO: Process execution and hart management
