use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid project: {0}")]
    InvalidProject(String),

    #[error("invalid scope format: {0}")]
    InvalidScope(String),

    #[error("entity not found: {entity} id={id}")]
    NotFound { entity: &'static str, id: String },

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("internal error: {0}")]
    Internal(String),
}
