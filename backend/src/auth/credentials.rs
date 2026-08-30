use crate::errors::AppError;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;

/// Derives a 32-byte master key via HKDF-SHA256 with domain separation (§100).
/// New format: HKDF(salt="", info="aerofs-credential-v1") with SHA-256.
/// Old SHA256(secret) is still accepted on decrypt for backward compat (key_version header).
pub fn derive_master_key(secret: &str) -> [u8; 32] {
    use hkdf::Hkdf;
    use sha2::Sha256;
    let hk = Hkdf::<Sha256>::new(None, secret.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(b"aerofs-credential-v1", &mut okm)
        .expect("HKDF expand failed");
    okm
}

/// Legacy SHA256 derivation for backward compat (pre-v1 ciphertexts)
pub fn derive_legacy_key(secret: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// Encrypts plaintext string with AES-256-GCM and returns versioned base64: `v1:<nonce+ciphertext>` (§101).
pub fn encrypt_secret(master_key: &[u8; 32], plaintext: &str) -> Result<String, AppError> {
    let cipher = Aes256Gcm::new_from_slice(master_key)
        .map_err(|e| anyhow::anyhow!("Cipher init error: {}", e))?;

    // 12-byte random nonce (never reuse, 96-bit per GCM spec)
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("Encryption error: {}", e))?;

    // Prepend nonce to ciphertext, then base64, then version header
    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    Ok(format!("v1:{}", BASE64.encode(&combined)))
}

/// Decrypts versioned `v1:<base64>` or legacy bare base64 with AES-256-GCM.
pub fn decrypt_secret(master_key: &[u8; 32], encoded: &str) -> Result<String, AppError> {
    // Strip version prefix if present; legacy ciphertexts have no prefix and were derived via SHA256
    let (key, payload) = if let Some(stripped) = encoded.strip_prefix("v1:") {
        (master_key, stripped)
    } else {
        // Legacy: no version header — ciphertext was created with old SHA256 key derivation.
        // We still try HKDF key first; if that fails, caller will retry with legacy key via fallback path.
        // For backward compat, we decode payload and attempt decrypt with HKDF key; if fails, outer
        // CredentialStore will retry with legacy key (see decrypt_secret_with_fallback below).
        (master_key, encoded)
    };

    let combined = BASE64
        .decode(payload)
        .map_err(|e| anyhow::anyhow!("Base64 decode error: {}", e))?;

    if combined.len() < 12 {
        return Err(anyhow::anyhow!("Invalid ciphertext length").into());
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| anyhow::anyhow!("Cipher init error: {}", e))?;

    let plaintext_bytes = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| anyhow::anyhow!("Decryption failed or invalid key"))?;

    let plaintext = String::from_utf8(plaintext_bytes)
        .map_err(|e| anyhow::anyhow!("UTF-8 decode error: {}", e))?;

    Ok(plaintext)
}

/// Try HKDF key first, then legacy SHA256 key for backward compat during rotation period.
pub fn decrypt_secret_with_legacy_fallback(
    primary_key: &[u8; 32],
    legacy_key: &[u8; 32],
    encoded: &str,
) -> Result<String, AppError> {
    match decrypt_secret(primary_key, encoded) {
        Ok(v) => Ok(v),
        Err(_) if !encoded.starts_with("v1:") => {
            // Legacy ciphertext — retry with old SHA256 derivation
            decrypt_secret(legacy_key, encoded)
        }
        Err(e) => Err(e),
    }
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
