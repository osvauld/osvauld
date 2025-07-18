use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certificate {
    pub private_key: String,
    pub public_key: String,
    pub salt: String,
}
