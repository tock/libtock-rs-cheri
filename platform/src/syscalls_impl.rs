//! Implements `Syscalls` for all types that implement `RawSyscalls`.

use crate::{
    allow_ro, allow_rw, exit_id, exit_on_drop, return_variant, share, subscribe, syscall_class,
    yield_id, AllowRo, AllowRw, CommandReturn, ErrorCode, RawSyscalls, Register, ReturnVariant,
    Subscribe, Syscalls, Upcall, YieldNoWaitReturn,
};
use kernel::cheri::cptr;

#[inline]
fn check_result(r0: Register, r1: Register) -> Result<(), ErrorCode> {
    let return_variant: ReturnVariant = r0.as_u32().into();
    // TRD 104 guarantees that Subscribe returns either Successor Failure.
    // We check the return variant by comparing against Failure for 2 reasons:
    //
    //   1. On RISC-V with compressed instructions, it generates smaller
    //      code. FAILURE_2_U32 has value 2, which can be loaded into a
    //      register with a single compressed instruction, whereas
    //      loading SUCCESS_2_U32 uses an uncompressed instruction.
    //   2. In the event the kernel malfuctions and returns a different
    //      return variant, the success path is actually safer than the
    //      failure path. The failure path assumes that r1 contains an
    //      ErrorCode, and produces UB if it has an out of range value.
    //      Incorrectly assuming the call succeeded will not generate
    //      unsoundness, and will likely lead to the application
    //      hanging.
    if return_variant == return_variant::FAILURE {
        // Safety: TRD 104 guarantees that if r0 is Failure with 2 U32,
        // then r1 will contain a valid error code. ErrorCode is
        // designed to be safely transmuted directly from a kernel error
        // code.
        return Err(unsafe { core::mem::transmute(r1.as_u32()) });
    }
    Ok(())
}

impl<S: RawSyscalls> Syscalls for S {
    // -------------------------------------------------------------------------
    // Yield
    // -------------------------------------------------------------------------

    fn yield_no_wait() -> YieldNoWaitReturn {
        let mut flag = core::mem::MaybeUninit::<YieldNoWaitReturn>::uninit();

        unsafe {
            // Flag can be uninitialized here because the kernel promises to
            // only write to it, not read from it. MaybeUninit guarantees that
            // it is safe to write a YieldNoWaitReturn into it.
            Self::yield2([yield_id::NO_WAIT.into(), flag.as_mut_ptr().into()]);

            // yield-no-wait guarantees it sets (initializes) flag before
            // returning.
            flag.assume_init()
        }
    }

    fn yield_wait() {
        // Safety: yield-wait does not return a value, which satisfies yield1's
        // requirement. The yield-wait system call cannot trigger undefined
        // behavior on its own in any other way.
        unsafe {
            Self::yield1([yield_id::WAIT.into()]);
        }
    }

    // -------------------------------------------------------------------------
    // Subscribe
    // -------------------------------------------------------------------------

    fn subscribe<
        'share,
        IDS: subscribe::SupportsId<DRIVER_NUM, SUBSCRIBE_NUM>,
        U: Upcall<IDS>,
        CONFIG: subscribe::Config,
        const DRIVER_NUM: u32,
        const SUBSCRIBE_NUM: u32,
    >(
        _subscribe: share::Handle<Subscribe<'share, Self, DRIVER_NUM, SUBSCRIBE_NUM>>,
        upcall: &'share U,
    ) -> Result<(), ErrorCode> {
        // The upcall function passed to the Tock kernel.
        //
        // Safety: data must be a reference to a valid instance of U.
        unsafe extern "C" fn kernel_upcall<S: Syscalls, IDS, U: Upcall<IDS>>(
            arg0: usize,
            arg1: usize,
            arg2: usize,
            data: Register,
        ) {
            let exit: exit_on_drop::ExitOnDrop<S> = Default::default();
            let upcall: *const U = data.into();
            unsafe { &*upcall }.upcall(arg0, arg1, arg2);
            core::mem::forget(exit);
        }

        // Inner function that does the majority of the work. This is not
        // monomorphized over DRIVER_NUM and SUBSCRIBE_NUM to keep code size
        // small.
        //
        // Safety: upcall_fcn must be kernel_upcall<S, IDS, U> and upcall_data
        // must be a reference to an instance of U that will remain valid as
        // long as the 'scope lifetime is alive. Can only be called if a
        // Subscribe<'scope, S, driver_num, subscribe_num> exists.
        unsafe fn inner<S: Syscalls, CONFIG: subscribe::Config>(
            driver_num: u32,
            subscribe_num: u32,
            upcall_fcn: Register,
            upcall_data: Register,
        ) -> Result<(), ErrorCode> {
            // Safety: syscall4's documentation indicates it can be used to call
            // Subscribe. These arguments follow TRD104. kernel_upcall has the
            // required signature. This function's preconditions mean that
            // upcall is a reference to an instance of U that will remain valid
            // until the 'scope lifetime is alive The existence of the
            // Subscribe<'scope, Self, DRIVER_NUM, SUBSCRIBE_NUM> guarantees
            // that if this Subscribe succeeds then the upcall will be cleaned
            // up before the 'scope lifetime ends, guaranteeing that upcall is
            // still alive when kernel_upcall is invoked.
            let [r0, r1, _, _] = unsafe {
                S::syscall4::<{ syscall_class::SUBSCRIBE }>([
                    driver_num.into(),
                    subscribe_num.into(),
                    upcall_fcn,
                    upcall_data,
                ])
            };

            check_result(r0, r1)?;

            // r0 indicates Success with 2 u32s. Confirm the null upcall was
            // returned, and it if wasn't then call the configured function.
            // We're relying on the optimizer to remove this branch if
            // returned_nonnull_upcall is a no-op.
            // Note: TRD 104 specifies that the null upcall has address 0,
            // not necessarily a null pointer.
            let returned_upcall: usize = r1.into();
            if returned_upcall != 0usize {
                CONFIG::returned_nonnull_upcall(driver_num, subscribe_num);
            }
            Ok(())
        }

        let upcall_fcn = Register::from_function(kernel_upcall::<S, IDS, U> as *const ());
        let upcall_data = (upcall as *const U).into();
        // Safety: upcall's type guarantees it is a reference to a U that will
        // remain valid for at least the 'scope lifetime. _subscribe is a
        // reference to a Subscribe<'scope, Self, DRIVER_NUM, SUBSCRIBE_NUM>,
        // proving one exists. upcall_fcn and upcall_data are derived in ways
        // that satisfy inner's requirements.
        unsafe { inner::<Self, CONFIG>(DRIVER_NUM, SUBSCRIBE_NUM, upcall_fcn, upcall_data) }
    }

    fn unsubscribe(driver_num: u32, subscribe_num: u32) {
        unsafe {
            // syscall4's documentation indicates it can be used to call
            // Subscribe. The upcall pointer passed is the null upcall, which
            // cannot cause undefined behavior on its own.
            Self::syscall4::<{ syscall_class::SUBSCRIBE }>([
                driver_num.into(),
                subscribe_num.into(),
                0usize.into(),
                0usize.into(),
            ]);
        }
    }

    // -------------------------------------------------------------------------
    // Command
    // -------------------------------------------------------------------------

    fn command(
        driver_id: u32,
        command_id: u32,
        argument0: usize,
        argument1: usize,
    ) -> CommandReturn {
        unsafe {
            // syscall4's documentation indicates it can be used to call
            // Command. The Command system call cannot trigger undefined
            // behavior on its own.
            let [r0, r1, r2, r3] = Self::syscall4::<{ syscall_class::COMMAND }>([
                driver_id.into(),
                command_id.into(),
                argument0.into(),
                argument1.into(),
            ]);

            // Because r0 and r1 are returned directly from the kernel, we are
            // guaranteed that if r0 represents a failure variant then r1 is an
            // error code.
            CommandReturn::new(r0.as_u32().into(), r1.into(), r2.into(), r3.into())
        }
    }

    // -------------------------------------------------------------------------
    // Read-Write Allow
    // -------------------------------------------------------------------------

    fn allow_rw<'share, CONFIG: allow_rw::Config, const DRIVER_NUM: u32, const BUFFER_NUM: u32>(
        _allow_rw: share::Handle<AllowRw<'share, Self, DRIVER_NUM, BUFFER_NUM>>,
        buffer: &'share mut [u8],
    ) -> Result<(), ErrorCode> {
        // Inner function that does the majority of the work. This is not
        // monomorphized over DRIVER_NUM and BUFFER_NUM to keep code size small.
        //
        // Safety: A share::Handle<AllowRw<'share, S, driver_num, buffer_num>>
        // must exist, and `buffer` must last for at least the 'share lifetime.
        unsafe fn inner<S: Syscalls, CONFIG: allow_rw::Config>(
            driver_num: u32,
            buffer_num: u32,
            buffer: &mut [u8],
        ) -> Result<(), ErrorCode> {
            // Safety: syscall4's documentation indicates it can be used to call
            // Read-Write Allow. These arguments follow TRD104.
            let [r0, r1, r2, _] = unsafe {
                S::syscall4::<{ syscall_class::ALLOW_RW }>([
                    driver_num.into(),
                    buffer_num.into(),
                    buffer.as_mut_ptr().into(),
                    buffer.len().into(),
                ])
            };

            check_result(r0, r1)?;

            // r0 indicates Success with 2 u32s. Confirm a zero buffer was
            // returned, and it if wasn't then call the configured function.
            // We're relying on the optimizer to remove this branch if
            // returned_nozero_buffer is a no-op.
            let returned_buffer: (usize, usize) = (r1.into(), r2.into());
            if returned_buffer != (0, 0) {
                CONFIG::returned_nonzero_buffer(driver_num, buffer_num);
            }
            Ok(())
        }

        // Safety: The presence of the share::Handle<AllowRw<'share, ...>>
        // guarantees that an AllowRw exists and will clean up this Allow ID
        // before the 'share lifetime ends.
        unsafe { inner::<Self, CONFIG>(DRIVER_NUM, BUFFER_NUM, buffer) }
    }

    fn unallow_rw(driver_num: u32, buffer_num: u32) -> Result<(), ErrorCode> {
        unsafe {
            // syscall4's documentation indicates it can be used to call
            // Read-Write Allow. The buffer passed has 0 length, which cannot
            // cause undefined behavior on its own.
            let [r0, r1, _, _] = Self::syscall4::<{ syscall_class::ALLOW_RW }>([
                driver_num.into(),
                buffer_num.into(),
                0usize.into(),
                0usize.into(),
            ]);
            check_result(r0, r1)
        }
    }

    // -------------------------------------------------------------------------
    // Read-Only Allow
    // -------------------------------------------------------------------------

    fn allow_ro<'share, CONFIG: allow_ro::Config, const DRIVER_NUM: u32, const BUFFER_NUM: u32>(
        _allow_ro: share::Handle<AllowRo<'share, Self, DRIVER_NUM, BUFFER_NUM>>,
        buffer: &'share [u8],
    ) -> Result<(), ErrorCode> {
        // Inner function that does the majority of the work. This is not
        // monomorphized over DRIVER_NUM and BUFFER_NUM to keep code size small.
        //
        // Security note: The syscall driver will retain read-only access to
        // `*buffer` until this Allow ID is unallowed or overwritten via another
        // Allow call. Therefore the caller must ensure the Allow ID is
        // unallowed or overwritten before `*buffer` is deallocated, to avoid
        // leaking newly-allocated information at the same address as `*buffer`.
        fn inner<S: Syscalls, CONFIG: allow_ro::Config>(
            driver_num: u32,
            buffer_num: u32,
            buffer: &[u8],
        ) -> Result<(), ErrorCode> {
            // Safety: syscall4's documentation indicates it can be used to call
            // Read-Only Allow. These arguments follow TRD104.
            let [r0, r1, r2, _] = unsafe {
                S::syscall4::<{ syscall_class::ALLOW_RO }>([
                    driver_num.into(),
                    buffer_num.into(),
                    buffer.as_ptr().into(),
                    buffer.len().into(),
                ])
            };

            check_result(r0, r1)?;

            // r0 indicates Success. Confirm a zero buffer was
            // returned, and it if wasn't then call the configured function.
            // We're relying on the optimizer to remove this branch if
            // returned_nozero_buffer is a no-op.
            let returned_buffer: (usize, usize) = (r1.into(), r2.into());
            if returned_buffer != (0, 0) {
                CONFIG::returned_nonzero_buffer(driver_num, buffer_num);
            }
            Ok(())
        }

        // Security: The presence of the share::Handle<AllowRo<'share, ...>>
        // guarantees that an AllowRo exists and will clean up this Allow ID
        // before the 'share lifetime ends.
        inner::<Self, CONFIG>(DRIVER_NUM, BUFFER_NUM, buffer)
    }

    fn unallow_ro(driver_num: u32, buffer_num: u32) -> Result<(), ErrorCode> {
        unsafe {
            // syscall4's documentation indicates it can be used to call
            // Read-Only Allow. The buffer passed has 0 length, which cannot
            // cause undefined behavior on its own.
            let [r0, r1, _, _] = Self::syscall4::<{ syscall_class::ALLOW_RO }>([
                driver_num.into(),
                buffer_num.into(),
                0usize.into(),
                0usize.into(),
            ]);
            check_result(r0, r1)
        }
    }

    // -------------------------------------------------------------------------
    // Exit
    // -------------------------------------------------------------------------

    fn exit_terminate(exit_code: u32) -> ! {
        unsafe {
            // syscall2's documentation indicates it can be used to call Exit.
            // The exit system call cannot trigger undefined behavior on its
            // own.
            Self::syscall2::<{ syscall_class::EXIT }>([
                exit_id::TERMINATE.into(),
                exit_code.into(),
            ]);
            // TRD104 indicates that exit-terminate MUST always succeed and so
            // never return.
            core::hint::unreachable_unchecked()
        }
    }

    fn exit_restart(exit_code: u32) -> ! {
        unsafe {
            // syscall2's documentation indicates it can be used to call Exit.
            // The exit system call cannot trigger undefined behavior on its
            // own.
            Self::syscall2::<{ syscall_class::EXIT }>([exit_id::RESTART.into(), exit_code.into()]);
            // TRD104 indicates that exit-restart MUST always succeed and so
            // never return.
            core::hint::unreachable_unchecked()
        }
    }

    fn memop(op_type: u32, arg1: usize) -> Result<cptr, ErrorCode> {
        unsafe {
            let [r0, r1] =
                Self::syscall2::<{ syscall_class::MEMOP }>([op_type.into(), arg1.into()]);
            let return_variant: ReturnVariant = r0.as_u32().into();
            if return_variant == return_variant::FAILURE {
                Err(core::mem::transmute(r1.as_u32()))
            } else {
                Ok(r1.0)
            }
        }
    }

    fn sbrk(offset: usize) -> Result<usize, ErrorCode> {
        Self::memop(1, offset).map(|ptr: cptr| {
            // On CHERI, sbrk should change DDC
            #[cfg(target_feature = "xcheri")]
            unsafe {
                core::arch::asm!(
                    "lc    ca0, 0(a0)",
                    "cspecialw ddc, ca0",
                    inlateout("a0") (& ptr as  *const cptr) => _,
                    options(preserves_flags, nostack),
                );
            }
            ptr.into()
        })
    }
}
