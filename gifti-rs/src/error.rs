use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GiftiError {
    #[error("I/O error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("malformed GIFTI XML: {0}")]
    Xml(#[from] roxmltree::Error),

    #[error("base64 decode failed: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("decompression failed: {0}")]
    Decompress(io::Error),

    #[error("{0}")]
    Format(String),
}

impl GiftiError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub(crate) fn fmt(msg: impl Into<String>) -> Self {
        Self::Format(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, GiftiError>;
