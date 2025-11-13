use super::capability::{ConnectionTokenType, Role};
use serde::{Deserialize, Serialize};
use std::result::Result as StdResult;
use ucan::Ucan;

/// Error type for connection token operations
#[derive(Debug)]
pub enum ConnectionTokenError {
    InvalidTokenType(String),
    ParsingFailed(String),
    MissingField(String),
    ValidationFailed(String),
}

impl std::fmt::Display for ConnectionTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionTokenError::InvalidTokenType(msg) => write!(f, "Invalid token type: {}", msg),
            ConnectionTokenError::ParsingFailed(msg) => write!(f, "Parsing failed: {}", msg),
            ConnectionTokenError::MissingField(msg) => write!(f, "Missing field: {}", msg),
            ConnectionTokenError::ValidationFailed(msg) => write!(f, "Validation failed: {}", msg),
        }
    }
}

impl std::error::Error for ConnectionTokenError {}

pub type ConnectionTokenResult<T> = StdResult<T, ConnectionTokenError>;

/// Parsed connection UCAN with domain logic
#[derive(Debug, Clone)]
pub struct ConnectionToken {
    raw_token: String,
    parsed: Ucan,
    token_type: ConnectionTokenType,
    role: Role,
}

impl ConnectionToken {
    /// Parse connection UCAN token
    pub fn from_token(token: &str) -> ConnectionTokenResult<Self> {
        // Parse UCAN token
        let parsed = Ucan::try_from(token)
            .map_err(|e| ConnectionTokenError::ParsingFailed(format!("UCAN parsing error: {}", e)))?;

        // Extract facts
        let facts = parsed.facts().as_ref().ok_or_else(|| {
            ConnectionTokenError::MissingField("UCAN facts not found".to_string())
        })?;

        // Extract token_type from facts
        let token_type_str = facts
            .get("token_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ConnectionTokenError::MissingField("token_type not found in facts".to_string()))?;

        let token_type = ConnectionTokenType::from_str(token_type_str)
            .map_err(|e| ConnectionTokenError::InvalidTokenType(e))?;

        // Extract role from facts
        let role_str = facts
            .get("role")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ConnectionTokenError::MissingField("role not found in facts".to_string()))?;

        let role = Role::from_str(role_str)
            .map_err(|e| ConnectionTokenError::ParsingFailed(format!("Invalid role: {}", e)))?;

        Ok(Self {
            raw_token: token.to_string(),
            parsed,
            token_type,
            role,
        })
    }

    // Accessors
    pub fn raw_token(&self) -> &str {
        &self.raw_token
    }

    pub fn token_type(&self) -> ConnectionTokenType {
        self.token_type
    }

    pub fn role(&self) -> Role {
        self.role
    }

    pub fn parsed(&self) -> &Ucan {
        &self.parsed
    }

    // Validation
    pub fn is_one_time(&self) -> bool {
        self.token_type.is_one_time()
    }

    pub fn can_delegate_to(&self, target_role: Role) -> bool {
        match self.token_type {
            ConnectionTokenType::OneTimeConnection => false, // One-time tokens can't delegate
            ConnectionTokenType::OwnerConnection => true,   // Owner can delegate to anyone
            ConnectionTokenType::NodeConnection => matches!(target_role, Role::User | Role::Viewer),
            ConnectionTokenType::UserConnection => matches!(target_role, Role::Viewer),
            ConnectionTokenType::ViewerAuth => false,        // Viewer can't delegate
        }
    }
}

/// One-time connection token (for initial handshake)
#[derive(Debug, Clone)]
pub struct OneTimeConnectionToken(ConnectionToken);

impl OneTimeConnectionToken {
    pub fn from_token(token: &str) -> ConnectionTokenResult<Self> {
        let conn = ConnectionToken::from_token(token)?;
        if conn.token_type() != ConnectionTokenType::OneTimeConnection {
            return Err(ConnectionTokenError::InvalidTokenType(
                format!("Expected OneTimeConnection, got {:?}", conn.token_type())
            ));
        }
        Ok(Self(conn))
    }

    pub fn inner(&self) -> &ConnectionToken {
        &self.0
    }
}

/// Owner connection token
#[derive(Debug, Clone)]
pub struct OwnerConnectionToken(ConnectionToken);

impl OwnerConnectionToken {
    pub fn from_token(token: &str) -> ConnectionTokenResult<Self> {
        let conn = ConnectionToken::from_token(token)?;
        if conn.token_type() != ConnectionTokenType::OwnerConnection {
            return Err(ConnectionTokenError::InvalidTokenType(
                format!("Expected OwnerConnection, got {:?}", conn.token_type())
            ));
        }
        if conn.role() != Role::Owner {
            return Err(ConnectionTokenError::ValidationFailed(
                format!("OwnerConnection must have Owner role, got {:?}", conn.role())
            ));
        }
        Ok(Self(conn))
    }

    pub fn inner(&self) -> &ConnectionToken {
        &self.0
    }
}

/// Node connection token
#[derive(Debug, Clone)]
pub struct NodeConnectionToken(ConnectionToken);

impl NodeConnectionToken {
    pub fn from_token(token: &str) -> ConnectionTokenResult<Self> {
        let conn = ConnectionToken::from_token(token)?;
        if conn.token_type() != ConnectionTokenType::NodeConnection {
            return Err(ConnectionTokenError::InvalidTokenType(
                format!("Expected NodeConnection, got {:?}", conn.token_type())
            ));
        }
        if conn.role() != Role::Node {
            return Err(ConnectionTokenError::ValidationFailed(
                format!("NodeConnection must have Node role, got {:?}", conn.role())
            ));
        }
        Ok(Self(conn))
    }

    pub fn inner(&self) -> &ConnectionToken {
        &self.0
    }
}

/// User connection token
#[derive(Debug, Clone)]
pub struct UserConnectionToken(ConnectionToken);

impl UserConnectionToken {
    pub fn from_token(token: &str) -> ConnectionTokenResult<Self> {
        let conn = ConnectionToken::from_token(token)?;
        if conn.token_type() != ConnectionTokenType::UserConnection {
            return Err(ConnectionTokenError::InvalidTokenType(
                format!("Expected UserConnection, got {:?}", conn.token_type())
            ));
        }
        if conn.role() != Role::User {
            return Err(ConnectionTokenError::ValidationFailed(
                format!("UserConnection must have User role, got {:?}", conn.role())
            ));
        }
        Ok(Self(conn))
    }

    pub fn inner(&self) -> &ConnectionToken {
        &self.0
    }
}

/// Viewer authentication token (from shareable link)
#[derive(Debug, Clone)]
pub struct ViewerAuthToken(ConnectionToken);

impl ViewerAuthToken {
    pub fn from_token(token: &str) -> ConnectionTokenResult<Self> {
        let conn = ConnectionToken::from_token(token)?;
        if conn.token_type() != ConnectionTokenType::ViewerAuth {
            return Err(ConnectionTokenError::InvalidTokenType(
                format!("Expected ViewerAuth, got {:?}", conn.token_type())
            ));
        }
        if conn.role() != Role::Viewer {
            return Err(ConnectionTokenError::ValidationFailed(
                format!("ViewerAuth must have Viewer role, got {:?}", conn.role())
            ));
        }
        Ok(Self(conn))
    }

    pub fn inner(&self) -> &ConnectionToken {
        &self.0
    }
}
