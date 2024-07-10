/* rt_header is defined by the general linker script (libtock_layout.ld). It has
 * the following layout:
 *
 *     Field                       | Offset
 *     ------------------------------------
 *     Top of the stack            |      0
 *     stack size                  |      4
 *     start of .bss               |      8
 *     Size of .bss                |     12
 *     start of relocations        |     16
 *     size of relocations         |     20
 */

.set STACK_TOP,  0
.set STACK_SIZE, 4
.set BSS_START,  8
.set BSS_SIZE,  12
.set REL_START, 16
.set REL_SIZE,  20

/* Store word on 32-bit, or double word on 64-bit */
.macro sx val, offset, base
  .if ARCH_BYTES == 4
    sw \val, \offset(\base)
  .else
    sd \val, \offset(\base)
  .endif
.endmacro


/* Load word on 32-bit, or double word on 64-bit */
.macro lx val, offset, base
  .if ARCH_BYTES == 4
    lw \val, \offset(\base)
  .else
    ld \val, \offset(\base)
  .endif
.endmacro

/* start is the entry point -- the first code executed by the kernel. The kernel
 * passes arguments through 4 registers:
 *
 *     a0  Pointer rt_header
 *
 *     a1  Address of the beginning of the process's usable memory region.
 *     a2  Size of the process' allocated memory region (including grant region)
 *     a3  Process break provided by the kernel.
 *
 */
.section .start, "ax"
.globl _start
_start:

.align 2

// This was just mostly copied from libtock-c crt0.
// TODO: merge them.

// Compute the stack top.
//
// struct hdr* myhdr = (struct hdr*) app_start;
// stacktop = mem_start + myhdr->stack_size + myhdr->stack_location

  lw   t0, STACK_SIZE(a0) // t0 = myhdr->stack_size
  lw   t1, STACK_TOP(a0)  // t1 = myhdr->stack_location
  add  t0, t0, a1
  add  t0, t0, t1

//
// Compute the app data size and where initial app brk should go.
// This includes the GOT, data, and BSS sections. However, we can't be sure
// the linker puts them back-to-back, but we do assume that BSS is last
// (i.e. myhdr->got_start < myhdr->bss_start && myhdr->data_start <
// myhdr->bss_start). With all of that true, then the size is equivalent
// to the end of the BSS section.
//
// app_brk = mem_start + myhdr->bss_start + myhdr->bss_size;

  lw   t1, BSS_START(a0)                // t1 = myhdr->bss_start
  lw   t2, BSS_SIZE(a0)                 // t2 = myhdr->bss_size
  add  s3, t1, a1                       // s3 = mem_start + bss_start
  add  s4, s3, t2                       // s4 = mem_start + bss_start + bss_size = app_brk
//
// Move arguments we need to keep over to callee-saved locations.
  mv   s0, a0                           // s0 = void* app_start
  mv   s1, t0                           // s1 = stack_top
  mv   s2, a1                           // s2 = mem_start

  mv  sp, t0                            // sp = stacktop
                                        // syscalls might use it
                                        // (Not currently true on RISCV.)

// We have overlapped the our BSS/HEAP with our relocations. If our
// relocations are larger, then we need to move the break to include
// relocations. Once we have processed relocations, we will move them
// back.

  lw  a0, REL_START(s0)
  lw  a1, REL_SIZE(s0)
  add a0, a0, s2          // a0 = reloc_start
  add t1, a0, a1          // a1 = reloc_end

  bgt  t1, s4, relocs_larger_than_bss
  mv   t1, s4
relocs_larger_than_bss:

// t1 is now the larger of the two

//
// Now we may want to move the stack pointer. If the kernel set the
// `app_heap_break` larger than we need (and we are going to call `brk()`
// to reduce it) then our stack pointer will fit and we can move it now.
// Otherwise after the first syscall (the memop to set the brk), the return
// will use a stack that is outside of the process accessible memory.
//
  ble t1, a3, skip_brk      // Compare `app_heap_break` (a3) with new brk.
                            // If our current `app_heap_break` is larger
                            // then there is no need to call brk at all.
                            // This happens when the relocations overlapping
                            // the stack are actually bigger than the stack +
                            // bss.
                            // Otherwise, we call brk to cover stack and bss.
                            // Heap is claimed later on the first call to
                            // malloc.

// Call `brk` to set to requested memory
// memop(0, stacktop + appdata_size);
  li  a4, 5               // a4 = 5   // memop syscall
  li  a0, 0               // a0 = 0
  mv  a1, t1              // a1 = app_brk
  ecall                   // memop

.if IS_CHERI
// On CHERI, brk returns a capability to authorise the new break
  // cspecialw ddc, ca1
  // for reason, I am getting a "there is no CHERI" exception here
  .byte 0x5b, 0x80, 0x15, 0x02
.endif

skip_brk:



//
// Debug support, tell the kernel the stack location
//
// memop(10, stacktop);
  li  a4, 5               // a4 = 5   // memop syscall
  li  a0, 10              // a0 = 10
  mv  a1, s1              // a1 = stacktop
  ecall                   // memop
//
// Debug support, tell the kernel the heap location
//
// memop(11, app_brk);
  li  a4, 5               // a4 = 5   // memop syscall
  li  a0, 11              // a0 = 11
  mv  a1, s4              // a1 = app_brk
  ecall                   // memop

// Process relocations. These have all been put in one segment for us and should
// be either Elf64_Rel or Elf32_Rel.

  .set r_offset, 0
  .set r_info, ARCH_BYTES
  .set ent_size, (ARCH_BYTES*2)

  lw  a0, REL_START(s0)
  lw  a1, REL_SIZE(s0)
  add a0, a0, s2          // a0 = reloc_start
  add a1, a0, a1          // a1 = reloc_end

  li  t0, 3               // t0 = R_RISCV_RELATIVE. The only relocation
                          // we should see.
  beq a0, a1, skip_loop
reloc_loop:
// Relocations are relative to a symbol, the table for which we have stripped.
// However, all the remaining relocations should use the special "0" symbol,
// and encode the values required in the addend.
  lx  a2, r_info, a0   // a2 = info
  lx  a3, r_offset, a0 // a3 = offset
  bne a2, t0, panic   // Only processing this relocation.
  add a3, a3, s2      // a3 = offset + reloc_offset
  lx   a4, 0, a3       // a4 = addend
  add a4, a4, s2      // a4 = addend + reloc_offset
  // Store new value
  sx  a4, 0, a3
skip_relocate:
  add a0, a0, ent_size
loop_footer:
  bne a0, a1, reloc_loop
skip_loop:

// Now relocations have been processed. If we moved our break too much, move it back.
// t1 still has the end of bss. a1 has the end of the relocs.

  bgt s4, a1, skip_second_brk
  li  a4, 5                  // a4 = 5   // memop syscall
  li  a0, 0                  // a0 = 0
  mv  a1, s4                 // a1 = app_brk
  ecall                      // memop
skip_second_brk:

  // We always do the clear because we may have used BSS for init
  // s3 has bss start, s4 has bss end
  beq  s3, s4, skip_zero_loop
  mv   a0, s3
zero_loop:
  sx    zero, 0, a0
  addi  a0, a0, ARCH_BYTES
  blt   a0, s4, zero_loop
skip_zero_loop:

.Lcall_rust_start:
	/* Note: rust_start must be a diverging function (i.e. return `!`) */
	jal rust_start

panic:
  lw  zero, 0(zero)