OUTPUT_ARCH( "riscv" )
ENTRY( _start )

SECTIONS
{
  /* Kernel starts at 0x80000000 in physical, but we load at 0x80100000 
     to leave room for OpenSBI/Bootloader if needed later. */
  . = 0x80100000;

  .text : {
    PROVIDE(_text_start = .);
    *(.text.entry)
    *(.text .text.*)
    PROVIDE(_text_end = .);
  }

  .rodata : {
    . = ALIGN(16);
    PROVIDE(_rodata_start = .);
    *(.rodata .rodata.*)
    PROVIDE(_rodata_end = .);
  }

  .data : {
    . = ALIGN(16);
    PROVIDE(_data_start = .);
    *(.data .data.*)
    PROVIDE(_data_end = .);
  }

  .bss : {
    . = ALIGN(16);
    PROVIDE(_bss_start = .);
    *(.bss .bss.*)
    *(COMMON)
    PROVIDE(_bss_end = .);
  }

  /* Define the end of the kernel. The Heap/PMM starts here. */
  . = ALIGN(4096);
  PROVIDE(_kernel_end = .);
}
