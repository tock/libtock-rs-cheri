#![forbid(unsafe_code)]
#![no_std]

extern crate libtock_debug_panic;

pub extern crate alloc;

pub mod prelude {
    // Normally these would be part of the prelude if using std. Re-exporting them here for better
    // compatibility. Programs still need to import libtock::prelude::*
    pub type Box<T> = alloc::boxed::Box<T>;
    pub type Vec<T> = alloc::vec::Vec<T>;
    pub type String = alloc::string::String;
    pub use libtock_runtime::print;
    pub use libtock_runtime::println;

    // Not a part of the std predule, but used by so many tock applications that we should also have
    // them in scope by default
    pub use libtock_runtime::set_main;
    pub use libtock_runtime::stack_size;
}

pub use libtock_platform as platform;
pub use libtock_runtime as runtime;

pub mod alarm {
    use libtock_alarm as alarm;
    pub type Alarm = alarm::Alarm<super::runtime::TockSyscalls>;
    pub use alarm::{Convert, Hz, Milliseconds, Ticks};
}
pub mod buttons {
    use libtock_buttons as buttons;
    pub type Buttons = buttons::Buttons<super::runtime::TockSyscalls>;
}
pub mod console {
    use libtock_console as console;
    pub type Console = console::Console<super::runtime::TockSyscalls>;
}
pub mod leds {
    use libtock_leds as leds;
    pub type Leds = leds::Leds<super::runtime::TockSyscalls>;
}
pub mod low_level_debug {
    use libtock_low_level_debug as lldb;
    pub type LowLevelDebug = lldb::LowLevelDebug<super::runtime::TockSyscalls>;
    pub use lldb::AlertCode;
}
