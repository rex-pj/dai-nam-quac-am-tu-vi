//! The application layer: it orchestrates ports, and knows neither SQL nor HTTP.
//!
//! Everything here depends on `Arc<dyn …Port>`, so it runs against a real repo or a fake one
//! alike. That is not abstraction for elegance: ranking policy and the orthography bridge are
//! the two places most likely to go wrong *as dictionary content*, and they must be testable
//! without standing up a database.

pub mod bridge;
pub mod service;

pub use bridge::{Bridge, Suggestion};
pub use service::{BrowseService, DictionaryService, EntryPage, SearchOutcome};

use thiserror::Error;

/// An error as the application layer sees it.
#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Repo(#[from] dnqatv_core::port::RepoError),

    #[error(transparent)]
    Domain(#[from] dnqatv_core::DomainError),

    #[error("invalid query")]
    BadQuery,
}
