ENTRY( boot )

EXTERN(status_summary);

PHDRS
{
  text PT_LOAD;
  data PT_LOAD;
  bss PT_LOAD;
}

__eh_frame_hdr_start = ADDR(.eh_frame_hdr);
__eh_frame_hdr_end = ADDR(.eh_frame_hdr) + SIZEOF(.eh_frame_hdr);
__eh_frame_start = ADDR(.eh_frame);
__eh_frame_end = ADDR(.eh_frame) + SIZEOF(.eh_frame);
__text_start = ADDR(.text);
__text_end = ADDR(.text) + SIZEOF(.text);



SECTIONS
{
  /* All text sections will go here */
  . = 0x80200000;
  .text ALIGN (4K) : AT (ADDR (.text))
  {
      *(.text.init)
      *(.text*)
  }
  .rodata ALIGN (4K) : AT (ADDR (.rodata))
  {
      *(.rodata)
      *(.rodata.*)
  }
  .data ALIGN (4K) : AT (ADDR (.data))
  {
      *(.sdata.*)
      *(.sdata)
      *(.data.*)
      *(.data)
      *(.pagetable)
  }

  .bss ALIGN (4K) : AT (ADDR (.bss))
  {
      *(COMMON)
      *(.sbss.*)
      *(.sbss)
      *(.bss.*)
      *(.bss)
      *(.bootstrap_stack)
  }
  .eh_frame_hdr ALIGN (4K) : AT (ADDR (.eh_frame_hdr))
  {
      *(.eh_frame_hdr)
  }
  .eh_frame ALIGN (4K) : AT (ADDR (.eh_frame))
  {
      *(.eh_frame)
  }
  . = ALIGN(4096);
  PROVIDE(_stack_end = .);
  
  /* Make 64kb of space for stack */
  /* Remember: stack grows downwards */
  . = . + 0x10000;
  . = ALIGN(4096);
  PROVIDE(_stack_start = .);
  PROVIDE(_heap_start = .);
  PROVIDE(_heap_end = 0x80000000+128M);
  PROVIDE(_free_space_start = 0x80000000+128M);
  PROVIDE(_uart_start = 0x10000000);
  PROVIDE(_virtio_start = 0x10001000);
  PROVIDE(_virtio_end = 0x10008000);
  
}
