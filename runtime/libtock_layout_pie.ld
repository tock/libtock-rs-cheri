/* Layout file for Tock process binaries that use libtock-rs. PIE version.

/* We need to use these magic values for ELF2TAB to work */

MEMORY {
    FLASH (rx) : ORIGIN = 0x80000000, LENGTH = 0x10000
    SRAM (RWX) : ORIGIN = 0x00000000, LENGTH = 0x10000
}

/* GNU LD looks for `start` as an entry point by default, while LLVM's LLD looks
 * for `_start`. To be compatible with both, we manually specify an entry point.
 */
ENTRY(_start)

WORD_ALIGN = 16;

/* TODO share this with libtock-c? */

SECTIONS {
    /* Sections located in FLASH to load from.
     */

    /* Reserve space for the stack. Aligned to a multiple of 16 bytes for the
     * RISC-V calling convention:
     * https://riscv.org/wp-content/uploads/2015/01/riscv-calling.pdf
     * It is important to have this first because so that overflows are more
     * likely to be caught
     */
    .stack ALIGN(16) (NOLOAD) : {
        _stack_start = .;
        KEEP(*(.stack_buffer))
        /* If app doesn't specify a stack size, use a default value of 0x1000 (4KiB) */
        . += . == _stack_start ? 0x1000 : 0x0;
        . = ALIGN(16);
        _stack_top = .;  /* Used in rt_header */
    } > SRAM

    /* Runtime header. Contains values the linker knows that the runtime needs
     * to look up.
     */
    .start : {
    	  /* _sram_origin is used by elf2tab:
	      * https://github.com/tock/elf2tab/blob/master/src/main.rs#L301
	      */
        _sram_origin = .;
        /* We combine rt_header and _start into a single section. If we don't,
         * elf2tab does not parse the ELF file correctly for unknown reasons.
         */
        rt_header = .;

        LONG(LOADADDR(.stack) - ORIGIN(SRAM));
        LONG(SIZEOF(.stack));
        LONG(LOADADDR(.bss) - ORIGIN(SRAM));
        LONG(SIZEOF(.bss));
        LONG(_data_rel_start - ORIGIN(SRAM));
        LONG(_data_rel_fake_end - _data_rel_fake_start);
        *(.start)
        . = ALIGN(WORD_ALIGN);
    } > SRAM AT > FLASH =0xFF

    /* Text section -- the application's code. */
    .text ALIGN(WORD_ALIGN) : {
        *(.text.*)
        *(.rodata.*)
        . = ALIGN(WORD_ALIGN);
    } > SRAM AT > FLASH =0xFF


    /* Sections located in RAM at runtime.
     */

    /* Read-write data section. This is deployed as part of FLASH but is copied
     * into RAM at runtime.
     */
    .data ALIGN(WORD_ALIGN) : {
        /* .sdata is the RISC-V small data section */
        *(.sdata .data)
        *(.data.*)
        /* GOT. Often none, but if the image gets too big this will be in use.
         * Should be close enough to the text so that it is in within immediate
         * range.
         */
        *(.got);
        . = ALIGN(WORD_ALIGN);
        /* These data_rel sections are the vtables */
        *(.data.rel.*);
        /* Because of the 4byte length field, if we are on an 8byte
         * platform, we need to purposefully mis-align by 4 bytes (see below).
         * On a 32-bit platform the following expression will align normally.
         */
        . += (4 - (. % WORD_ALIGN)) % WORD_ALIGN;
    } >SRAM AT > FLASH

    /* End of flash. */
    .endflash :
    {
    } > FLASH

    /* Working around is ELF2TAB becoming tiresome at this point. How it
     * currently works:
     *    ELF2 incorrectly uses section headers, not segment headers, to load
     *    data in the resulting binary. Putting the rel in a PT_LOAD segment
     *    therefore has no effect.
     *    ON THE OTHER HAND, ELF2TAB will find any section named exactly "rel.X"
     *    (where X is the name of any other section that has both W and R flags)
     *    and (ignoring where they request being placed) will chuck that section
     *    out at the end of the binary, preceded by a 4-byte length field.
     * This will result in the .rel.X section always being placed after
     * "endofflash". We have to produce the section, but not increment
     * the cursor because we actually want it to overlap where the stack
     * and BSS would be. Overlapping the STACK / BSS allows us to reclaim
     * ram once relocations have been processed. This works because no
     * relocations target the stack/BSS (yet another reason to use rel, not
     * rela).
     */

    _data_rel_start = . + 4;
    /* Must be called .rel.data */
    .rel.data : {
        /* Dynamic relocations. We should not get any rel.plt. Hopefully. */
        _data_rel_fake_start = .;
        KEEP(*(.rel.dyn*));
        _data_rel_fake_end = .;
    } = 0xaa

    . = _data_rel_start;

    /* BSS section. These are zero-initialized static variables. This section is
     * not copied from FLASH into RAM but rather directly initialized, and is
     * mainly put in this linker script so that we get an error if it overflows
     * the RAM region.
     */
    .bss ALIGN(WORD_ALIGN) (NOLOAD) : {
        /* .sbss is the RISC-V small data section */
        KEEP(*(.sbss .bss.*));
	. = ALIGN(WORD_ALIGN);
    } > SRAM


    /* TODO: heap via ELF2TAB or here? Could do it the same way as stack size */

    /* Sections we do not need. */
    /DISCARD/ :
    {
      *(.ARM.exidx .eh_frame .eh_frame_hdr .gnu.hash .dynsym .dynstr .hash .dynamic)
    }
}
