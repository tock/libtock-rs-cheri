//! Some macros and types commonly found in std for Tock to improve compatibility

/// Print to the console followed by a newline character
#[macro_export]
macro_rules! println {
    ($($ts: tt)*) => {
      {
        use core::fmt::Write;
        writeln!(libtock::console::Console::writer(), $($ts)*).unwrap()
      }
    };
}

/// Print to the console
#[macro_export]
macro_rules! print {
    ($($ts: tt)*) => {
      {
        use core::fmt::Write;
        write!(libtock::console::Console::writer(), $($ts)*).unwrap()
      }
    };
}
