use crate::{
    allow_ro, allow_rw, share, subscribe, AllowRo, AllowRw, CommandReturn, ErrorCode, RawSyscalls,
    Subscribe, Upcall, YieldNoWaitReturn,
};
use kernel::cheri::cptr;

/// `Syscalls` provides safe abstractions over Tock's system calls. It is
/// implemented for `libtock_runtime::TockSyscalls` and
/// `libtock_unittest::fake::Kernel` (by way of `RawSyscalls`).
pub trait Syscalls: RawSyscalls + Sized {
    // -------------------------------------------------------------------------
    // Yield
    // -------------------------------------------------------------------------

    /// Runs the next pending callback, if a callback is pending. Unlike
    /// `yield_wait`, `yield_no_wait` returns immediately if no callback is
    /// pending.
    fn yield_no_wait() -> YieldNoWaitReturn;

    /// Puts the process to sleep until a callback becomes pending, invokes the
    /// callback, then returns.
    fn yield_wait();

    // -------------------------------------------------------------------------
    // Subscribe
    // -------------------------------------------------------------------------

    /// Registers an upcall with the kernel.
    fn subscribe<
        'share,
        IDS: subscribe::SupportsId<DRIVER_NUM, SUBSCRIBE_NUM>,
        U: Upcall<IDS>,
        CONFIG: subscribe::Config,
        const DRIVER_NUM: u32,
        const SUBSCRIBE_NUM: u32,
    >(
        subscribe: share::Handle<Subscribe<'share, Self, DRIVER_NUM, SUBSCRIBE_NUM>>,
        upcall: &'share U,
    ) -> Result<(), ErrorCode>;

    /// Unregisters the upcall with the given ID. If no upcall is registered
    /// with the given ID, `unsubscribe` does nothing.
    fn unsubscribe(driver_num: u32, subscribe_num: u32);

    // -------------------------------------------------------------------------
    // Command
    // -------------------------------------------------------------------------

    fn command(
        driver_id: u32,
        command_id: u32,
        argument0: usize,
        argument1: usize,
    ) -> CommandReturn;

    // -------------------------------------------------------------------------
    // Read-Write Allow
    // -------------------------------------------------------------------------

    /// Shares a read-write buffer with the kernel.
    fn allow_rw<'share, CONFIG: allow_rw::Config, const DRIVER_NUM: u32, const BUFFER_NUM: u32>(
        allow_rw: share::Handle<AllowRw<'share, Self, DRIVER_NUM, BUFFER_NUM>>,
        buffer: &'share mut [u8],
    ) -> Result<(), ErrorCode>;

    /// Revokes the kernel's access to the buffer with the given ID, overwriting
    /// it with a zero buffer. If no buffer is shared with the given ID,
    /// `unallow_rw` does nothing.
    fn unallow_rw(driver_num: u32, buffer_num: u32) -> Result<(), ErrorCode>;

    // -------------------------------------------------------------------------
    // Read-Only Allow
    // -------------------------------------------------------------------------

    /// Shares a read-only buffer with the kernel.
    fn allow_ro<'share, CONFIG: allow_ro::Config, const DRIVER_NUM: u32, const BUFFER_NUM: u32>(
        allow_ro: share::Handle<AllowRo<'share, Self, DRIVER_NUM, BUFFER_NUM>>,
        buffer: &'share [u8],
    ) -> Result<(), ErrorCode>;

    fn allow_ro_32<
        'share,
        CONFIG: allow_ro::Config,
        const DRIVER_NUM: u32,
        const BUFFER_NUM: u32,
    >(
        allow_ro: share::Handle<AllowRo<'share, Self, DRIVER_NUM, BUFFER_NUM>>,
        buffer: &'share [u32],
    ) -> Result<(), ErrorCode> {
        // Safety: every buffer of u32s is also a buffer of u8s
        Self::allow_ro::<CONFIG, DRIVER_NUM, BUFFER_NUM>(allow_ro, unsafe {
            let len = core::mem::size_of::<u32>() * buffer.len();
            let ptr = buffer.as_ptr() as *const u8;
            core::slice::from_raw_parts(ptr, len)
        })
    }

    /// Revokes the kernel's access to the buffer with the given ID, overwriting
    /// it with a zero buffer. If no buffer is shared with the given ID,
    /// `unallow_ro` does nothing.
    fn unallow_ro(driver_num: u32, buffer_num: u32) -> Result<(), ErrorCode>;

    /// Perform a memory operation
    fn memop(op_type: u32, arg1: usize) -> Result<cptr, ErrorCode>;

    /// Move the user/kernel break by offset bytes.
    /// Returns the ABSOLUTE address of the previous user/kernel break.
    /// On CHERI: DDC will be automatically set to authorise at least up to the new break.
    fn sbrk(offset: usize) -> Result<usize, ErrorCode>;

    /// TODO: wrap the other memops

    // -------------------------------------------------------------------------
    // Exit
    // -------------------------------------------------------------------------

    fn exit_terminate(exit_code: u32) -> !;

    fn exit_restart(exit_code: u32) -> !;
}
