// Emit `cfg=diary_sqlite` when the SQLite backend should be active. The
// rule is: mobile target (android, ios) ALWAYS wins; otherwise the user
// opts in with `--features sqlite` (used by desktop tests of the SQLite
// repository). Postgres is the implicit fallback so `cargo build` on
// macOS / Linux / Windows keeps the old behaviour.
//
// We can't pick the backend purely with cargo features because Tauri's
// `tauri android dev` only knows how to *add* features (no
// `--no-default-features`), and the desktop `default = ["postgres"]`
// drags the postgres driver in. So both drivers may compile under
// android — only one type alias resolves and dead code in the other is
// dropped at link.

fn main() {
    // Tell cargo this is a known cfg flag so #[cfg(diary_sqlite)] won't
    // trip the unexpected-cfg lint on Rust 1.80+.
    println!("cargo:rustc-check-cfg=cfg(diary_sqlite)");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let force_sqlite = std::env::var_os("CARGO_FEATURE_SQLITE").is_some();
    let is_mobile = target_os == "android" || target_os == "ios";

    if force_sqlite || is_mobile {
        println!("cargo:rustc-cfg=diary_sqlite");
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_SQLITE");

    tauri_build::build()
}
