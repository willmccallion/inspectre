ENTRY(_start)

SECTIONS {
  /* User programs are loaded at 0x80200000 */
  . = 0x80200000;

  .text : {
    *(.text .text.*)
  }

  .rodata : {
    . = ALIGN(8);
    *(.rodata .rodata.*)
  }

  .data : {
    . = ALIGN(8);
    /* Define gp for relaxation optimizations */
    PROVIDE(__global_pointer$ = . + 0x800);
    *(.data .data.*)
    *(.sdata .sdata.*)
  }

  /* BSS Section: Must be zeroed by crt0.s */
  .bss : {
    . = ALIGN(8);
    _bss_start = .;
    *(.bss .bss.*)
    *(.sbss .sbss.*)
    *(COMMON)
    . = ALIGN(8);
    _bss_end = .;
  }

  . = ALIGN(8);
  _end = .;
}
