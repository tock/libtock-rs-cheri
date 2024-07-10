use core::arch::asm;
use libtock_platform::cheri::cptr;
use libtock_platform::{RawSyscalls, Register};

/* For CHERI, it would be preferable just to pass all registers with a "cx" constraint
 * But, without more compiler support, we cannot pass capabilities to asm blocks.
 * instead, we have to pass references to the register and then load.
 * Because this generates extra logic, I have kept the times this is done to only be arguments
 * that might actually contain capabilities. This is the second argument of yield,
 * and the last two of syscall4.
 */

unsafe impl RawSyscalls for crate::TockSyscalls {
    // This yield implementation is currently limited to RISC-V versions without
    // floating-point registers, as it does not mark them clobbered.
    #[cfg(not(any(target_feature = "d", target_feature = "f")))]
    unsafe fn yield1([Register(r0)]: [Register; 1]) {
        // Safety: This matches the invariants required by the documentation on
        // RawSyscalls::yield1
        unsafe {
            asm!("ecall",
                 // x0 is the zero register.
                 lateout("x1") _, // Return address
                 // x2-x4 are stack, global, and thread pointers. sp is
                 // callee-saved.
                 lateout("x5") _, // t0
                 lateout("x6") _, // t1
                 lateout("x7") _, // t2
                 // x8 and x9 are s0 and s1 and are callee-saved.
                 inlateout("x10") <cptr as Into<usize>>::into(r0) => _, // a0
                 lateout("x11") _,         // a1
                 lateout("x12") _,         // a2
                 lateout("x13") _,         // a3
                 inlateout("x14") 0 => _,  // a4
                 lateout("x15") _,         // a5
                 lateout("x16") _,         // a6
                 lateout("x17") _,         // a7
                 // x18-27 are s2-s11 and are callee-saved
                 lateout("x28") _, // t3
                 lateout("x29") _, // t4
                 lateout("x30") _, // t5
                 lateout("x31") _, // t6
            );
        }
    }

    // This yield implementation is currently limited to RISC-V versions without
    // floating-point registers, as it does not mark them clobbered.
    #[cfg(not(any(target_feature = "d", target_feature = "f")))]
    unsafe fn yield2([Register(r0), Register(r1)]: [Register; 2]) {
        // Safety: This matches the invariants required by the documentation on
        // RawSyscalls::yield2
        unsafe {
            #[cfg(not(target_feature = "xcheri"))]
            asm!("ecall",
                 // x0 is the zero register.
                 lateout("x1") _, // Return address
                 // x2-x4 are stack, global, and thread pointers. sp is
                 // callee-saved.
                 lateout("x5") _, // t0
                 lateout("x6") _, // t1
                 lateout("x7") _, // t2
                 // x8 and x9 are s0 and s1 and are callee-saved.
                 inlateout("x10") r0 => _, // a0
                 inlateout("x11") r1 => _, // a1
                 lateout("x12") _,         // a2
                 lateout("x13") _,         // a3
                 inlateout("x14") 0 => _,  // a4
                 lateout("x15") _,         // a5
                 lateout("x16") _,         // a6
                 lateout("x17") _,         // a7
                 // x18-27 are s2-s11 and are callee-saved
                 lateout("x28") _, // t3
                 lateout("x29") _, // t4
                 lateout("x30") _, // t5
                 lateout("x31") _, // t6
            );
            #[cfg(target_feature = "xcheri")]
            {
                let mut r1 = r1;
                asm!("lc c11, (x11)",
                     "ecall",
                     // x0 is the zero register.
                     lateout("x1") _, // Return address
                     // x2-x4 are stack, global, and thread pointers. sp is
                     // callee-saved.
                     lateout("x5") _, // t0
                     lateout("x6") _, // t1
                     lateout("x7") _, // t2
                     // x8 and x9 are s0 and s1 and are callee-saved.
                     inlateout("x10") <cptr as Into<usize>>::into(r0) => _, // a0
                     inlateout("x11") &mut r1 => _, // a1
                     lateout("x12") _,         // a2
                     lateout("x13") _,         // a3
                     inlateout("x14") 0 => _,  // a4
                     lateout("x15") _,         // a5
                     lateout("x16") _,         // a6
                     lateout("x17") _,         // a7
                     // x18-27 are s2-s11 and are callee-saved
                     lateout("x28") _, // t3
                     lateout("x29") _, // t4
                     lateout("x30") _, // t5
                     lateout("x31") _, // t6
                );
            }
        }
    }

    unsafe fn syscall1<const CLASS: usize>([Register(r0)]: [Register; 1]) -> [Register; 2] {
        let mut r0 = <cptr as Into<usize>>::into(r0);
        let r1: usize;
        // Safety: This matches the invariants required by the documentation on
        // RawSyscalls::syscall1
        unsafe {
            asm!("ecall",
                 inlateout("a0") r0,
                 lateout("a1") r1,
                 in("a4") CLASS,
                 options(preserves_flags, nostack, nomem),
            );
        }
        [Register(r0.into()), Register(r1.into())]
    }

    unsafe fn syscall2<const CLASS: usize>(
        [Register(r0), Register(mut r1)]: [Register; 2],
    ) -> [Register; 2] {
        // Safety: This matches the invariants required by the documentation on
        // RawSyscalls::syscall2
        let mut r0 = <cptr as Into<usize>>::into(r0);
        unsafe {
            #[cfg(not(target_feature = "xcheri"))]
            asm!("ecall",
                 inlateout("a0") r0,
                 inlateout("a1") r1,
                 in("a4") CLASS,
                 options(preserves_flags, nostack, nomem)
            );
            // Syscall2 needs to maintain the second argument return as a capability because it
            // is used for the sbrk memop
            #[cfg(target_feature = "xcheri")]
            asm!("lc    ca1, 0(a5)",
                 "ecall",
                 "sc    ca1, 0(a5)",
                 inlateout("a0") r0,
                 inlateout("a5") &mut r1 => _,
                 lateout("a1") _,
                 in("a4") CLASS,
                 options(preserves_flags, nostack),
            );
        }
        [Register(r0.into()), Register(r1)]
    }

    unsafe fn syscall4<const CLASS: usize>(
        [Register(r0), Register(r1), Register(mut r2), Register(mut r3)]: [Register; 4],
    ) -> [Register; 4] {
        // Safety: This matches the invariants required by the documentation on
        // RawSyscalls::syscall4
        let mut r0 = <cptr as Into<usize>>::into(r0);
        let mut r1 = <cptr as Into<usize>>::into(r1);
        unsafe {
            #[cfg(not(target_feature = "xcheri"))]
            asm!("ecall",
                 inlateout("a0") r0,
                 inlateout("a1") r1,
                 inlateout("a2") r2,
                 inlateout("a3") r3,
                 in("a4") CLASS,
                 options(preserves_flags, nostack),
            );
            #[cfg(target_feature = "xcheri")]
            asm!("lc    ca2, 0(a5)",
                 "lc    ca3, 0(a6)",
                 "ecall",
                 "sc    ca2, 0(a5)",
                 "sc    ca3, 0(a6)",
                 inlateout("a0") r0,
                 inlateout("a1") r1,
                 inlateout("a5") &mut r2 => _,
                 inlateout("a6") &mut r3 => _,
                 lateout("a2") _,
                 lateout("a3") _,
                 in("a4") CLASS,
                 options(preserves_flags, nostack),
            );
        }
        [
            Register(r0.into()),
            Register(r1.into()),
            Register(r2),
            Register(r3),
        ]
    }
}
