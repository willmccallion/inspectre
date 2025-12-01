.option norvc
.section .text
    .global _start
    .extern main
    .extern _bss_start
    .extern _bss_end
    .extern __global_pointer$

_start:
    # ------------------------------------------------
    # 1. Initialize Global Pointer (gp)
    #    Required for -O3 optimizations
    # ------------------------------------------------
.option push
.option norelax
    la gp, __global_pointer$
.option pop

    # ------------------------------------------------
    # 2. Zero out the BSS section
    #    This ensures variables like heap_top are 0
    # ------------------------------------------------
    la t0, _bss_start
    la t1, _bss_end
    
    # If start >= end, skip (empty BSS)
    bge t0, t1, bss_clear_done

bss_clear_loop:
    sd zero, 0(t0)        # Write 0 to memory
    addi t0, t0, 8        # Move to next 8 bytes
    blt t0, t1, bss_clear_loop

bss_clear_done:

    # ------------------------------------------------
    # 3. Setup Stack and Call Main
    # ------------------------------------------------
    # The kernel sets SP, but we align it for ABI compliance
    andi sp, sp, -16

    # Call main(argc=0, argv=NULL)
    li a0, 0
    li a1, 0
    call main

    # ------------------------------------------------
    # 4. Exit
    # ------------------------------------------------
    # Return code is in a0 from main
    li a7, 93             # Syscall: Exit
    ecall

# Infinite loop if syscall fails
loop:
    j loop
