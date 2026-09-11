//! Environment variable sources, hidden behind a trait.
//!
//! **This is not abstraction for its own sake.** Since edition 2024 `std::env::set_var` is
//! `unsafe`, and this whole workspace declares `unsafe_code = "forbid"` — so tests *cannot*
//! build an environment by setting variables. Even if they could, the process environment is
//! shared global state and `cargo test` runs in parallel, so two tests setting the same key
//! would collide.
//!
//! Extracting a trait is the only remaining way to stay testable without opening the door to
//! `unsafe`. This follows §14 of the plan: whatever needs testing is made **legitimately
//! public**.

use std::collections::BTreeMap;

pub trait EnvSource {
    fn get(&self, key: &str) -> Option<String>;
}

/// The real process environment.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemEnv;

impl EnvSource for SystemEnv {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

/// An in-memory environment — used by tests, and for loading a `.env` file.
#[derive(Debug, Clone, Default)]
pub struct MapEnv(BTreeMap<String, String>);

impl MapEnv {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with(mut self, key: &str, value: &str) -> Self {
        self.0.insert(key.to_owned(), value.to_owned());
        self
    }

    pub fn insert(&mut self, key: &str, value: &str) {
        self.0.insert(key.to_owned(), value.to_owned());
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl EnvSource for MapEnv {
    fn get(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
}

/// Reads the first source, falling back to the second when a key is absent.
///
/// Used to let the real environment **beat** the `.env` file — the 12-factor convention: a
/// `.env` file is a developer-machine convenience, while in deployment the real environment
/// is the source of truth.
pub struct Layered<A, B>(pub A, pub B);

impl<A: EnvSource, B: EnvSource> EnvSource for Layered<A, B> {
    fn get(&self, key: &str) -> Option<String> {
        self.0.get(key).or_else(|| self.1.get(key))
    }
}
