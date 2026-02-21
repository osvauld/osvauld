use thiserror::Error;

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("token parse failed: {0}")]
    TokenParse(String),
    #[error("token missing facts")]
    MissingFacts,
    #[error("policy facts missing: expected 'osv_policy'")]
    MissingPolicyFacts,
    #[error("facts decode failed: {0}")]
    FactsDecode(String),
    #[error("validation failed: {0}")]
    Validation(String),
}
