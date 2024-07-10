// auto_layout() identifies the correct linker scripts to use based on the
// LIBTOCK_PLATFORM environment variable, and copies the linker scripts into
// OUT_DIR. The cargo invocation must pass -C link-arg=-Tlayout.ld to rustc
// (using the rustflags cargo config).
#[cfg(not(feature = "no_auto_layout"))]
fn auto_layout() {
    use std::fs::copy;
    use std::path::PathBuf;

    const PLATFORM_CFG_VAR: &str = "LIBTOCK_PLATFORM";
    const LAYOUT_GENERIC_FILENAME: &str = "libtock_layout_pie.ld";

    // Note: we need to print these rerun-if commands before using the variable
    // or file, so that if the build script fails cargo knows when to re-run it.
    println!("cargo:rerun-if-env-changed={}", PLATFORM_CFG_VAR);

    // Read configuration from environment variables.

    // Note: cargo fails if run in a path that is not valid Unicode, so this
    // script doesn't need to handle non-Unicode paths. Also, OUT_DIR cannot be
    // in a location with a newline in it, or we have no way to pass
    // rustc-link-search to cargo.
    let out_dir = &std::env::var("OUT_DIR").expect("Unable to read OUT_DIR");
    assert!(
        !out_dir.contains('\n'),
        "Build path contains a newline, which is unsupported"
    );

    // Copy the generic layout file into OUT_DIR.
    let out_layout_generic: PathBuf = [out_dir, LAYOUT_GENERIC_FILENAME].iter().collect();
    println!("cargo:rerun-if-changed={}", LAYOUT_GENERIC_FILENAME);
    copy(LAYOUT_GENERIC_FILENAME, out_layout_generic)
        .expect("Unable to copy layout_generic.ld into OUT_DIR");

    // Link in libc. Only needed for malloc.
    println!(
        "cargo:rustc-link-search=native={}/lib",
        std::env::var("CHERI_LIBC").expect("CHERI_LIBC not set"),
    );
    println!("cargo:rustc-link-lib=static=c");

    // Tell rustc where to search for the layout file.
    println!("cargo:rustc-link-search={}", out_dir);
}

fn main() {
    #[cfg(not(feature = "no_auto_layout"))]
    auto_layout();
}
