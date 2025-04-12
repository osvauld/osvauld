use serde::{Deserialize, Serialize};

pub struct GeneratedKeys {
    pub private_key: String,
    pub public_key: String,
    pub salt: String,
}

#[derive(Serialize, Deserialize)]
pub struct PasswordChangeInput {
    pub old_password: String,
    pub new_password: String,
    pub enc_pvt_key: String,
    pub salt: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EncryptedResource {
    pub encrypted_data: String,
    pub encrypted_key: String,
}
