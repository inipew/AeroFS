use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use crate::errors::AppError;

/// Derives a 32-byte master key from an input secret string (using SHA-256)
pub fn derive_master_key(secret: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// Encrypts plaintext string with AES-256-GCM and returns a base64-encoded string: nonce + ciphertext
pub fn encrypt_secret(master_key: &[u8; 32], plaintext: &str) -> Result<String, AppError> {
    let cipher = Aes256Gcm::new_from_slice(master_key)
        .map_err(|e| anyhow::anyhow!("Cipher init error: {}", e))?;

    // 12-byte random nonce
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("Encryption error: {}", e))?;

    // Prepend nonce to ciphertext
    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    Ok(BASE64.encode(&combined))
}

/// Decrypts base64-encoded ciphertext with AES-256-GCM
pub fn decrypt_secret(master_key: &[u8; 32], encoded: &str) -> Result<String, AppError> {
    let combined = BASE64
        .decode(encoded)
        .map_err(|e| anyhow::anyhow!("Base64 decode error: {}", e))?;

    if combined.len() < 12 {
        return Err(anyhow::anyhow!("Invalid ciphertext length").into());
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = Aes256Gcm::new_from_slice(master_key)
        .map_err(|e| anyhow::anyhow!("Cipher init error: {}", e))?;

    let plaintext_bytes = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| anyhow::anyhow!("Decryption failed or invalid key"))?;

    let plaintext = String::from_utf8(plaintext_bytes)
        .map_err(|e| anyhow::anyhow!("UTF-8 decode error: {}", e))?;

    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_encryption_roundtrip() {
        let key = derive_master_key("my_super_secure_master_password_123");
        let secret = "sftp_super_secret_ssh_private_key_or_password";

        let encrypted = encrypt_secret(&key, secret).unwrap();
        assert_ne!(secret, encrypted);

        let decrypted = decrypt_secret(&key, &encrypted).unwrap();
        assert_eq!(secret, decrypted);
    }

    #[test]
    fn test_credential_wrong_key_fails() {
        let key1 = derive_master_key("key_one");
        let key2 = derive_master_key("key_two");
        let secret = "password123";

        let encrypted = encrypt_secret(&key1, secret).unwrap();
        let res = decrypt_secret(&key2, &encrypted);
        assert!(res.is_err());
    }
}
