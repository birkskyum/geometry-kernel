use thiserror::Error;

pub type Result<T> = std::result::Result<T, GeometryError>;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum GeometryError {
    #[error("invalid geometry: {0}")]
    InvalidGeometry(String),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
    #[error("backend error: {0}")]
    Backend(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

impl From<serde_json::Error> for GeometryError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialization(value.to_string())
    }
}
