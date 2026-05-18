use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

#[derive(Debug, Error, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "code", content = "message")]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn i18n_key(&self) -> &'static str {
        match self {
            AppError::NotFound(_) => "errors.notFound",
            AppError::Database(_) => "errors.database",
            AppError::Validation(_) => "errors.validation",
            AppError::Internal(_) => "errors.internal",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_tagged_json() {
        let err = AppError::NotFound("meeting m-999".into());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, r#"{"code":"notFound","message":"meeting m-999"}"#);
    }

    #[test]
    fn each_variant_has_i18n_key() {
        assert_eq!(AppError::NotFound("x".into()).i18n_key(), "errors.notFound");
        assert_eq!(AppError::Database("x".into()).i18n_key(), "errors.database");
        assert_eq!(
            AppError::Validation("x".into()).i18n_key(),
            "errors.validation"
        );
        assert_eq!(AppError::Internal("x".into()).i18n_key(), "errors.internal");
    }
}
