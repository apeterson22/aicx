use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AicxError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("encoding error: {0}")]
    Encode(#[from] serde_cbor::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("archive validation error: {0}")]
    Validation(String),
    #[error("unsafe path: {0}")]
    UnsafePath(String),
    #[error("unsupported codec: {0}")]
    UnsupportedCodec(String),
    #[error("overwrite denied for existing path: {0}")]
    OverwriteDenied(String),
    #[error("selection produced no archive paths")]
    EmptySelection,
}

pub type Result<T> = std::result::Result<T, AicxError>;
