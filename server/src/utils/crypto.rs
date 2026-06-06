#![allow(deprecated)]

use chacha20poly1305::aead::generic_array::GenericArray;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, KeySizeUser, OsRng};
use chacha20poly1305::ChaCha20Poly1305;
use chacha20poly1305::Nonce;
use secrecy::{ExposeSecret, SecretBox};
use typenum::Unsigned;

use crate::infra::errors::InfraError;

pub const KEY_LEN: usize = <ChaCha20Poly1305 as KeySizeUser>::KeySize::USIZE;
const NONCE_LEN: usize = <ChaCha20Poly1305 as AeadCore>::NonceSize::USIZE;

pub fn encrypt(plaintext: &[u8], key: &SecretBox<[u8; KEY_LEN]>) -> Result<Vec<u8>, InfraError> {
    let cipher = ChaCha20Poly1305::new(GenericArray::from_slice(key.expose_secret()));
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|_| InfraError::InternalServerError)?;
    let mut result = vec![0; NONCE_LEN + ciphertext.len()];
    result[..NONCE_LEN].copy_from_slice(&nonce);
    result[NONCE_LEN..].copy_from_slice(&ciphertext);
    Ok(result)
}

pub fn decrypt(payload: &[u8], key: &SecretBox<[u8; KEY_LEN]>) -> Result<Vec<u8>, InfraError> {
    if payload.len() < NONCE_LEN {
        return Err(InfraError::InternalServerError);
    }
    let cipher = ChaCha20Poly1305::new(GenericArray::from_slice(key.expose_secret()));
    let mut nonce_arr = [0u8; NONCE_LEN];
    nonce_arr.copy_from_slice(&payload[..NONCE_LEN]);
    let nonce = Nonce::from(nonce_arr);
    cipher
        .decrypt(&nonce, &payload[NONCE_LEN..])
        .map_err(|_| InfraError::InternalServerError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = ChaCha20Poly1305::generate_key(&mut OsRng);
        let key_arr: [u8; KEY_LEN] = key.into();
        let secret_key = SecretBox::new(Box::new(key_arr));

        let plaintext = b"hello world this is a test message";
        let encrypted = encrypt(plaintext, &secret_key).unwrap();
        let decrypted = decrypt(&encrypted, &secret_key).unwrap();

        assert_eq!(decrypted, plaintext);
    }
}
