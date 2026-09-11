//! Reads a `.env` file into a [`MapEnv`] — it does **not** touch the process environment.
//!
//! The popular `dotenvy` crate works by calling `std::env::set_var`. Since edition 2024 that
//! function is `unsafe`, and this workspace sets `forbid(unsafe_code)`; beyond that, mutating
//! the global environment at runtime is neither observable nor undoable.
//!
//! Here the file is only **read as data**, then layered underneath the real environment via
//! [`crate::source::Layered`]. Where a value came from stays visible, and tests need no
//! special privileges.

use std::path::Path;

use crate::source::MapEnv;

/// Parse the contents of a `.env` file.
///
/// Accepts: blank lines, `# comment`, an `export ` prefix, and values wrapped in `'` or `"`.
/// Lines without `=` are skipped — a `.env` file is hand-typed, one stray line should not kill
/// the program, and every required key is still checked in [`crate::EnvVar::read`].
pub fn parse(text: &str) -> MapEnv {
    let mut out = MapEnv::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        out.insert(key, unquote(value.trim()));
    }
    out
}

fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {
            return &value[1..value.len() - 1];
        }
    }
    value
}

/// Read the file if it exists. **A missing file is not an error** — in deployment the
/// configuration comes from the real environment, where having no `.env` is normal.
pub fn load(path: &Path) -> MapEnv {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(_) => MapEnv::new(),
    }
}
