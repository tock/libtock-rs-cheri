//! Tock heap allocation
//!

extern crate alloc;

use crate::TockSyscalls;
use core::alloc::{GlobalAlloc, Layout};
use libtock_low_level_debug::{AlertCode, LowLevelDebug};
use libtock_platform::{ErrorCode, Syscalls};

/// An allocator that defers to malloc in libc. This adds libc as a dependency.
struct TockAllocatorMalloc;

// The functions we pull in from malloc in libc
extern "C" {
    fn free(ptr: *mut u8);
    fn memalign(align: usize, nbytes: usize) -> *mut u8;
    // These are also available but not currently in use:
    //fn malloc(nbytes : usize) -> *mut u8;
    //fn realloc(ptr : *mut u8, nbytes : usize) -> *mut u8;
}

/// Provide the C-ABI sbrk in case those function use it.
#[no_mangle]
pub extern "C" fn _sbrk(nbytes: usize) -> *mut u8 {
    (match TockSyscalls::sbrk(nbytes) {
        Ok(ptr) => ptr,
        Err(_) => 0usize,
    }) as *mut u8
}

// And an alias for picolib
#[no_mangle]
pub extern "C" fn sbrk(nbytes: usize) -> *mut u8 {
    _sbrk(nbytes)
}

unsafe impl GlobalAlloc for TockAllocatorMalloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { memalign(layout.align(), layout.size()) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe { free(ptr) }
    }
}

#[alloc_error_handler]
fn oom_handler(_layout: core::alloc::Layout) -> ! {
    LowLevelDebug::<TockSyscalls>::print_alert_code(AlertCode::HeapOOM);
    TockSyscalls::exit_terminate(ErrorCode::NoMem as u32);
}

#[global_allocator]
static GLOBAL: TockAllocatorMalloc = TockAllocatorMalloc;
