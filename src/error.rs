use axum::{
    http::{Error as HttpError, StatusCode},
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Not Found")]
    NotFound,

    #[error("Internal Server Error")]
    Internal(#[from] anyhow::Error),

    #[error("I/O Error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid Range Header")]
    InvalidRange,

    #[error("HTTP error: {0}")]
    Http(#[from] HttpError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::InvalidRange => (StatusCode::RANGE_NOT_SATISFIABLE, self.to_string()),
            AppError::Internal(_) | AppError::Io(_) | AppError::Http(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
        };

        (status, message).into_response()
    }
}