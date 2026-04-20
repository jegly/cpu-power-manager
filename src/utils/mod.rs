// Utility modules
pub mod error;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CpuError {
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Invalid value: {0}")]
    InvalidValue(String),
    #[error("Not supported: {0}")]
    NotSupported(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
