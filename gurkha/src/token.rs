// V3 Migration: Simplified token module - no typed wrappers
//
// We use a single `Permit` type (in parser.rs) for all tokens.
// Facts determine behavior, not wrapper types.
//
// Usage: our_permit, their_permit

use std::result::Result as StdResult;

// ============================================================================
// Error Types
// ============================================================================

/// Error type for UCAN token operations
#[derive(Debug)]
pub enum UcanTokenError {
    InvalidTokenType(String),
    ParsingFailed(String),
    MissingField(String),
    ValidationFailed(String),
}

impl std::fmt::Display for UcanTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UcanTokenError::InvalidTokenType(msg) => write!(f, "Invalid token type: {}", msg),
            UcanTokenError::ParsingFailed(msg) => write!(f, "Parsing failed: {}", msg),
            UcanTokenError::MissingField(msg) => write!(f, "Missing field: {}", msg),
            UcanTokenError::ValidationFailed(msg) => write!(f, "Validation failed: {}", msg),
        }
    }
}

impl std::error::Error for UcanTokenError {}

pub type UcanTokenResult<T> = StdResult<T, UcanTokenError>;

// ============================================================================
// All token wrappers removed - use Permit directly from parser.rs
// ============================================================================
//
// V3 Architecture:
// - One type: Permit (in parser.rs)
// - Facts determine behavior
// - No rigid type system
// - Fully data-driven
//
// Example usage:
//   let our_permit = Permit::from_token(&token)?;
//   if our_permit.is_owner() { ... }
//   if their_permit.resource_id() == Some("xyz") { ... }
