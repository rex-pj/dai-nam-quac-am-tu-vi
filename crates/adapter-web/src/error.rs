//! Web-layer errors.
//!
//! Principle: **the message for the user and the message for the log are two different
//! strings.** Someone looking up a word does not need a table name or a SQL statement; an
//! operator does. Mixing the two is how infrastructure detail leaks onto the internet.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug, thiserror::Error)]
pub enum WebError {
    #[error("not found")]
    NotFound,

    #[error("invalid request: {0}")]
    BadRequest(String),

    #[error("render failed: {0}")]
    Render(String),

    #[error("application layer error: {0}")]
    App(#[from] dnqatv_app::AppError),
}

impl WebError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Render(_) | Self::App(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// What the user is told. Never technical detail — and **Vietnamese, because a reader reads it**.
    pub fn public_message(&self) -> &'static str {
        match self {
            Self::NotFound => "Không tìm thấy trang này.",
            Self::BadRequest(_) => "Yêu cầu không hợp lệ.",
            Self::Render(_) | Self::App(_) => {
                "Có trục trặc ở máy chủ. Chỗ hỏng đã được biên lại; xin thử lại sau."
            }
        }
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        // Details go to the server log, never to the browser.
        if self.status().is_server_error() {
            eprintln!("[error] {self}");
        }
        (self.status(), self.public_message()).into_response()
    }
}
