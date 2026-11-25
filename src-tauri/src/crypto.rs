//! Small, self-contained encryption helpers: AES-256-GCM with a PBKDF2-derived key.
//! Format of an encrypted line: base64( 16-byte salt || 12-byte nonce || ciphertext ).

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::RngCore;
use sha2::Sha256;

const PBKDF2_ROUNDS: u32 = 100_000;

fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, PBKDF2_ROUNDS, &mut key);
    key
}

pub fn encrypt_string(password: &str, plaintext: &str) -> Result<String, String> {
    let mut salt = [0u8; 16];
    let mut nonce_bytes = [0u8; 12];
    let mut rng = rand::thread_rng();
    rng.fill_bytes(&mut salt);
    rng.fill_bytes(&mut nonce_bytes);

    let key = derive_key(password, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext.as_bytes())
        .map_err(|_| "encryption failed".to_string())?;

    let mut blob = Vec::with_capacity(16 + 12 + ciphertext.len());
    blob.extend_from_slice(&salt);
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(blob))
}

pub fn decrypt_string(password: &str, encoded: &str) -> Result<String, String> {
    let blob = STANDARD
        .decode(encoded.trim())
        .map_err(|_| "invalid base64".to_string())?;
    if blob.len() < 28 {
        return Err("encrypted record too short".into());
    }
    let key = derive_key(password, &blob[..16]);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
    let plain = cipher
        .decrypt(Nonce::from_slice(&blob[16..28]), &blob[28..])
        .map_err(|_| "wrong password or corrupted record".to_string())?;
    String::from_utf8(plain).map_err(|_| "decrypted data is not UTF-8".into())
}

pub fn random_password(len: usize) -> String {
    const ALPHANUMERIC: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..len.max(8))
        .map(|_| ALPHANUMERIC[rng.next_u32() as usize % ALPHANUMERIC.len()] as char)
        .collect()
}
