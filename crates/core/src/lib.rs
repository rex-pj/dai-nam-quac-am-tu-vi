//! Business domain of Đại Nam Quấc Âm Tự Vị.
//!
//! This crate does NOT depend on sea-orm, axum or any infrastructure — that boundary is
//! enforced by Cargo's dependency graph, not by convention.

pub mod error;
pub mod model;
pub mod port;
pub mod search;
pub mod text;

pub use error::DomainError;
