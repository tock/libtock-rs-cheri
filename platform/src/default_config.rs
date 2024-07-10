/// A general purpose syscall configuration, which drivers should use as their
/// default syscall config.
pub struct DefaultConfig;

impl crate::allow_ro::Config for DefaultConfig {}
impl crate::allow_rw::Config for DefaultConfig {}
impl crate::subscribe::Config for DefaultConfig {}

/// Combo trait for all configs
pub trait AllConfig:
    crate::allow_ro::Config + crate::allow_rw::Config + crate::subscribe::Config
{
}
impl<T: crate::allow_ro::Config + crate::allow_rw::Config + crate::subscribe::Config> AllConfig
    for T
{
}
