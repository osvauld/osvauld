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

pub struct UserPublicKey {
    pub user_id: String,
    pub public_key: String,
    pub access: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UserAccess {
    pub user_id: String,
    pub access: String,
    pub encrypted_key: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EncryptedDataWithAccess {
    pub encrypted_data: String,
    pub access_list: Vec<UserAccess>,
}

#[derive(Serialize, Deserialize, Debug)]

pub struct CredentialWithEncryptedKey {
    pub id: String,
    pub credential_type: String,
    pub data: String,
    pub signature: String,
    pub encrypted_key: String,
    pub last_accessed: i64,
    pub favourite: bool,
    pub folder_id: String,
}

#[derive(Serialize, Deserialize, Debug)]

pub struct EncryptedCredential {
    pub encrypted_data: String,
    pub encrypted_key: String,
}
