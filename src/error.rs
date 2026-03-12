use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, UncleFunkleError>;

#[derive(Debug, Error)]
pub enum UncleFunkleError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid state: {0}")]
    InvalidState(String),
    #[error("task join error: {0}")]
    Join(String),
}

impl UncleFunkleError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
