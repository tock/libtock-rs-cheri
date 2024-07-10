use kernel::cheri::cptr;
use kernel::cheri::CPtrOps;

/// In order to work with Miri's `-Zmiri-track-raw-pointers` flag, we cannot
/// pass pointers to the kernel through `usize` values (as casting to and from
/// `usize` drops the pointer`s tag). Instead, `RawSyscalls` uses the `Register`
/// type. `Register` wraps a raw pointer type that keeps that tags around. User
/// code should not depend on the particular type of pointer that `Register`
/// wraps, but instead use the conversion functions in this module.
// Register is repr(transparent) so that an upcall's application data can be
// soundly passed as a Register.
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct Register(pub cptr);

// -----------------------------------------------------------------------------
// Conversions to Register
// -----------------------------------------------------------------------------

impl From<crate::ErrorCode> for Register {
    fn from(value: crate::ErrorCode) -> Register {
        (value as usize).into()
    }
}

impl From<u32> for Register {
    fn from(value: u32) -> Register {
        (value as usize).into()
    }
}

impl From<usize> for Register {
    fn from(value: usize) -> Register {
        Register(value.into())
    }
}

impl<T> From<*mut T> for Register {
    fn from(value: *mut T) -> Register {
        (value as *const T).into()
    }
}

impl<T> From<*const T> for Register {
    fn from(value: *const T) -> Register {
        let mut v: cptr = Default::default();
        // We don't use core::mem::size_of::<T>() as a length here because users of this interface
        // lie about the type of T too much.
        v.set_addr_from_ddc(value as usize);
        Register(v)
    }
}

// -----------------------------------------------------------------------------
// Infallible conversions from Register
// -----------------------------------------------------------------------------

// If we implement From<u32> on Register, then we automatically get a
// TryFrom<Error = Infallible> implementation, which conflicts with our fallible
// TryFrom implementation. We could choose to not implement TryFrom and instead
// add a fallible accessor (something like "expect_u32"), but that seems
// confusing. Instead, we use an inherent method for the Register -> u32
// infallible conversion.
impl Register {
    /// Casts this register to a u32, truncating it if it is larger than
    /// u32::MAX. This conversion should be avoided in host-based test code; use
    /// the `TryFrom<Register> for u32` implementation instead.
    pub fn as_u32(self) -> u32 {
        let as_usize: usize = self.0.into();
        as_usize as u32
    }

    /// Similar to From<*const ()> but on CHERI will derive from PCC
    pub fn from_function(fnptr: *const ()) -> Register {
        let mut v: cptr = Default::default();
        v.set_addr_from_pcc(fnptr as usize);
        Register(v)
    }
}

impl From<Register> for usize {
    fn from(register: Register) -> usize {
        register.0.into()
    }
}

impl<T> From<Register> for *mut T {
    fn from(register: Register) -> *mut T {
        <Register as Into<*const T>>::into(register) as *mut T
    }
}

impl<T> From<Register> for *const T {
    fn from(register: Register) -> *const T {
        // Again, we can't check the CHERI length here because the user can cast to an arbitrary
        // *const T to a *const V, which may have different size.
        let as_usize: usize = register.into();
        as_usize as *const T
    }
}

// -----------------------------------------------------------------------------
// Fallible conversions from Register
// -----------------------------------------------------------------------------

/// Converts a `Register` to a `u32`. Returns an error if the `Register`'s value
/// is larger than `u32::MAX`. This is intended for use in host-based tests; in
/// Tock process binary code, use Register::as_u32 instead.
impl TryFrom<Register> for u32 {
    type Error = core::num::TryFromIntError;

    fn try_from(register: Register) -> Result<u32, core::num::TryFromIntError> {
        let as_usize: usize = register.into();
        (as_usize).try_into()
    }
}
