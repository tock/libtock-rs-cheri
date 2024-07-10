use crate::ErrorCode;
use crate::share::List;
use crate::Syscalls;

// -----------------------------------------------------------------------------
// `Subscribe` struct
// -----------------------------------------------------------------------------

/// A `Subscribe` instance allows safe code to call Tock's Subscribe system
/// call, by guaranteeing the upcall will be cleaned up before 'share ends. It
/// is generally used with the `share::scope` function, which offers a safe
/// interface for constructing `Subscribe` instances.
pub struct Subscribe<'share, S: Syscalls, const DRIVER_NUM: u32, const SUBSCRIBE_NUM: u32> {
    _syscalls: core::marker::PhantomData<S>,

    // Make this struct invariant with respect to the 'share lifetime.
    //
    // Covariance would be unsound, as that would allow code with a
    // `Subscribe<'static, ...>` to register an upcall that lasts for a shorter
    // lifetime, resulting in use-after-free if the upcall is invoked.
    // Contravariance would be sound, but is not necessary and may be confusing.
    //
    // Additionally, we want to have at least one private member of this struct
    // so that code outside this module cannot construct a `Subscribe` without
    // calling `ShareList::new`.
    _scope: core::marker::PhantomData<core::cell::Cell<&'share ()>>,
}

// We can't derive(Default) because S is not Default, and derive(Default)
// generates a Default implementation that requires S to be Default. Instead, we
// manually implement Default.
impl<'share, S: Syscalls, const DRIVER_NUM: u32, const SUBSCRIBE_NUM: u32> Default
    for Subscribe<'share, S, DRIVER_NUM, SUBSCRIBE_NUM>
{
    fn default() -> Self {
        Self {
            _syscalls: Default::default(),
            _scope: Default::default(),
        }
    }
}

impl<'share, S: Syscalls, const DRIVER_NUM: u32, const SUBSCRIBE_NUM: u32> Drop
    for Subscribe<'share, S, DRIVER_NUM, SUBSCRIBE_NUM>
{
    fn drop(&mut self) {
        S::unsubscribe(DRIVER_NUM, SUBSCRIBE_NUM);
    }
}

impl<'share, S: Syscalls, const DRIVER_NUM: u32, const SUBSCRIBE_NUM: u32> List
    for Subscribe<'share, S, DRIVER_NUM, SUBSCRIBE_NUM>
{
}

// -----------------------------------------------------------------------------
// `Upcall` trait
// -----------------------------------------------------------------------------

/// A Tock kernel upcall. Upcalls are registered using the Subscribe system
/// call, and are invoked during Yield calls.
///
/// Each `Upcall` supports one or more subscribe IDs, which are indicated by the
/// `SupportedIds` parameter. The types `AnySubscribeId` and `OneSubscribeId`
/// are provided to use as `SupportedIds` parameters in `Upcall`
/// implementations.
pub trait Upcall<SupportedIds> {
    fn upcall(&self, arg0: usize, arg1: usize, arg2: usize);
}

pub trait SupportsId<const DRIVER_NUM: u32, const SUBSCRIBE_NUM: u32> {}

pub struct AnyId;
impl<const DRIVER_NUM: u32, const SUBSCRIBE_NUM: u32> SupportsId<DRIVER_NUM, SUBSCRIBE_NUM>
    for AnyId
{
}

pub struct OneId<const DRIVER_NUM: u32, const SUBSCRIBE_NUM: u32>;
impl<const DRIVER_NUM: u32, const SUBSCRIBE_NUM: u32> SupportsId<DRIVER_NUM, SUBSCRIBE_NUM>
    for OneId<DRIVER_NUM, SUBSCRIBE_NUM>
{
}

// -----------------------------------------------------------------------------
// Upcall implementations that simply store their arguments
// -----------------------------------------------------------------------------

/// An implementation of `Upcall` that sets the contained boolean value to
/// `true` when the upcall is invoked.
impl Upcall<AnyId> for core::cell::Cell<bool> {
    fn upcall(&self, _: usize, _: usize, _: usize) {
        self.set(true);
    }
}

/// Implemented for consistency with the other `Cell<Option<...>>` `Upcall`
/// impls. Most users would prefer the `Cell<bool>` implementation over this
/// impl, but this may be useful in a generic or macro context.
impl Upcall<AnyId> for core::cell::Cell<Option<()>> {
    fn upcall(&self, _: usize, _: usize, _: usize) {
        self.set(Some(()));
    }
}

/// An `Upcall` implementation that stores its first argument when called.
impl Upcall<AnyId> for core::cell::Cell<Option<(usize,)>> {
    fn upcall(&self, arg0: usize, _: usize, _: usize) {
        self.set(Some((arg0,)));
    }
}

/// An `Upcall` implementation that stores its first two arguments when called.
impl Upcall<AnyId> for core::cell::Cell<Option<(usize, usize)>> {
    fn upcall(&self, arg0: usize, arg1: usize, _: usize) {
        self.set(Some((arg0, arg1)));
    }
}

/// An `Upcall` implementation that stores its arguments when called.
impl Upcall<AnyId> for core::cell::Cell<Option<(usize, usize, usize)>> {
    fn upcall(&self, arg0: usize, arg1: usize, arg2: usize) {
        self.set(Some((arg0, arg1, arg2)));
    }
}

/// An `Upcall` implementation that stores its first argument as u32 when called.
impl Upcall<AnyId> for core::cell::Cell<Option<(u32,)>> {
    fn upcall(&self, arg0: usize, _: usize, _: usize) {
        self.set(Some((arg0 as u32,)));
    }
}

/// An `Upcall` implementation that stores its first two arguments as u32 when called.
impl Upcall<AnyId> for core::cell::Cell<Option<(u32, u32)>> {
    fn upcall(&self, arg0: usize, arg1: usize, _: usize) {
        self.set(Some((arg0 as u32, arg1 as u32)));
    }
}

/// An `Upcall` implementation that stores its arguments as u32 when called.
impl Upcall<AnyId> for core::cell::Cell<Option<(u32, u32, u32)>> {
    fn upcall(&self, arg0: usize, arg1: usize, arg2: usize) {
        self.set(Some((arg0 as u32, arg1 as u32, arg2 as u32)));
    }
}

/// Expose the types uses here with more useful names
pub type StandardResult = core::cell::Cell<Option<(usize,)>>;
pub type StandardResultArg1 = core::cell::Cell<Option<(usize, usize)>>;
pub type StandardResultArg2 = core::cell::Cell<Option<(usize, usize, usize)>>;

pub trait UpcallResult {
    type Arg;
    /// Get the upcall result as a Result<> type.
    /// None if the upcall has not been called yet.
    fn upcall_result(&self) -> Option<Result<Self::Arg, ErrorCode>>;

    /// Get the upcall result as a Result<> type, yielding if the upcall has not been called.
    #[inline]
    fn upcall_result_yield<S: Syscalls>(&self) -> Result<Self::Arg, ErrorCode> {
        loop {
            if let Some(result) = self.upcall_result() {
                return result;
            }
            S::yield_wait();
        }
    }

    #[inline]
    fn reset(&self) {}
}

macro_rules! implement_result {
    ($t : ty, $out : ty, {$($args : tt)*}, $($outputs : tt)*) => {
        impl UpcallResult for $t {
            type Arg = $out;

            #[inline]
            fn upcall_result(&self) -> Option<Result<Self::Arg, ErrorCode>> {
                match self.get() {
                    None => {None}
                    Some((x,$($args)*)) => {
                        Some (match x {
                            0 => Ok($($outputs)*),
                            err => Err((err as u32).try_into().unwrap_or(ErrorCode::Fail)),
                        })
                    }
                }
            }

            #[inline]
            fn reset(&self) {
                self.set(None);
            }
        }

    };
}

implement_result!(StandardResult, (), {}, ());
implement_result!(StandardResultArg1, usize, { y }, y);
implement_result!(StandardResultArg2, (usize, usize), {y, z}, (y, z));

#[cfg(test)]
#[test]
fn upcall_impls() {
    let cell_bool = core::cell::Cell::new(false);
    cell_bool.upcall(1, 2, 3);
    assert!(cell_bool.get());

    let cell_empty = core::cell::Cell::new(None);
    cell_empty.upcall(1, 2, 3);
    assert_eq!(cell_empty.get(), Some(()));

    let cell_one = core::cell::Cell::new(None);
    cell_one.upcall(1, 2, 3);
    assert_eq!(cell_one.get(), Some((1,)));

    let cell_two = core::cell::Cell::new(None);
    cell_two.upcall(1, 2, 3);
    assert_eq!(cell_two.get(), Some((1, 2)));

    let cell_three = core::cell::Cell::new(None);
    cell_three.upcall(1, 2, 3);
    assert_eq!(cell_three.get(), Some((1, 2, 3)));
}

// -----------------------------------------------------------------------------
// `Config` trait
// -----------------------------------------------------------------------------

/// `Config` configures the behavior of the Subscribe system call. It should
/// generally be passed through by drivers, to allow application code to
/// configure error handling.
pub trait Config {
    /// Called if a Subscribe call succeeds and returns a non-null upcall. In
    /// some applications, this may indicate unexpected reentrance. By default,
    /// the non-null upcall is ignored.
    fn returned_nonnull_upcall(_driver_num: u32, _subscribe_num: u32) {}
}
