use crate::auth::credentials::{decrypt_secret, derive_master_key, encrypt_secret};
use crate::errors::AppError;

#[derive(Debug, Clone)]
pub struct CredentialStore {
    master_key: [u8; 32],
    legacy_key: [u8; 32],
}

impl CredentialStore {
    pub fn new(session_secret: &str) -> Self {
        let master_key = derive_master_key(session_secret);
        let legacy_key = crate::auth::credentials::derive_legacy_key(session_secret);
        // Note: derive_legacy_key is private; expose via crate::auth::credentials
        // For now derive via same SHA256 inline if not accessible
        Self {
            master_key,
            legacy_key,
        }
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String, AppError> {
        encrypt_secret(&self.master_key, plaintext)
    }

    pub fn decrypt(&self, ciphertext: &str) -> Result<String, AppError> {
        // Try HKDF key first, then legacy SHA256 for backward compat
        match decrypt_secret(&self.master_key, ciphertext) {
            Ok(v) => Ok(v),
            Err(_) if !ciphertext.starts_with("v1:") => {
                crate::auth::credentials::decrypt_secret(&self.legacy_key, ciphertext)
            }
            Err(e) => Err(e),
        }
    }
}
