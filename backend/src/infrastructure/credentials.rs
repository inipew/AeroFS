use crate::auth::credentials::{decrypt_secret, derive_master_key, encrypt_secret};
use crate::errors::AppError;

#[derive(Debug, Clone)]
pub struct CredentialStore {
    master_key: [u8; 32],
}

impl CredentialStore {
    pub fn new(session_secret: &str) -> Self {
        let master_key = derive_master_key(session_secret);
        Self { master_key }
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String, AppError> {
        encrypt_secret(&self.master_key, plaintext)
    }

    pub fn decrypt(&self, ciphertext: &str) -> Result<String, AppError> {
        decrypt_secret(&self.master_key, ciphertext)
    }
}
