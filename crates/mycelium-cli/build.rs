//! Build script for the `mycelium` binary.
//!
//! On Windows, raises the linker stack size to 8 MiB so the main thread
//! can initialise all 11 tree-sitter parsers without crashing with
//! `STATUS_STACK_OVERFLOW` (0xC00000FD). Linux and macOS already default
//! to 8 MiB, so this is a no-op there.
//!
//! We use `rustc-link-arg-bin` (per-binary linker arg) instead of
//! `.cargo/config.toml` rustflags because workflows set
//! `RUSTFLAGS=-D warnings` at the env level, which fully overrides
//! `.cargo/config.toml`'s `rustflags` (cargo concatenates them only when
//! both come from config files, not when env wins).

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // 8 MiB = 0x800000 = 8388608. MSVC linker syntax.
        println!("cargo:rustc-link-arg-bin=mycelium=/STACK:8388608");
        // GNU-style (mingw/MSYS) fallback.
        println!("cargo:rustc-link-arg-bin=mycelium=-Wl,--stack,8388608");
    }
}
