use libsodium_rs::{crypto_box, crypto_sign, ensure_init};

pub const KEY_LEN: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum EncryptionError {
    #[error("libsodium init failed")]
    SodiumInitFailed,
    #[error("invalid ed25519 public key for x25519 conversion")]
    InvalidEd25519PublicKey,
    #[error("invalid ed25519 secret key for x25519 conversion")]
    InvalidEd25519SecretKey,
    #[error("invalid x25519 public key")]
    InvalidX25519PublicKey,
    #[error("invalid x25519 secret key")]
    InvalidX25519SecretKey,
    #[error("failed to encrypt payload")]
    SealFailed,
    #[error("failed to decrypt payload")]
    OpenFailed,
}

fn init_sodium() -> Result<(), EncryptionError> {
    ensure_init().map_err(|_| EncryptionError::SodiumInitFailed)?;
    Ok(())
}

/// Convert an Ed25519 public key into an X25519 public key.
pub fn ed25519_pubkey_to_x25519(
    ed25519_pubkey: &[u8; KEY_LEN],
) -> Result<[u8; KEY_LEN], EncryptionError> {
    init_sodium()?;
    let ed_pk = crypto_sign::PublicKey::from_bytes(ed25519_pubkey)
        .map_err(|_| EncryptionError::InvalidEd25519PublicKey)?;
    let x_pk = crypto_sign::ed25519_pk_to_curve25519(&ed_pk)
        .map_err(|_| EncryptionError::InvalidEd25519PublicKey)?;
    let mut out = [0u8; KEY_LEN];
    out.copy_from_slice(&x_pk);
    Ok(out)
}

/// Convert an Ed25519 secret key into an X25519 secret key.
pub fn ed25519_secret_to_x25519(
    ed25519_secret_key: &[u8],
) -> Result<[u8; KEY_LEN], EncryptionError> {
    assert_eq!(ed25519_secret_key.len(), 64);

    init_sodium()?;
    let ed_sk = crypto_sign::SecretKey::from_bytes(ed25519_secret_key)
        .map_err(|_| EncryptionError::InvalidEd25519SecretKey)?;
    let x_sk = crypto_sign::ed25519_sk_to_curve25519(&ed_sk)
        .map_err(|_| EncryptionError::InvalidEd25519SecretKey)?;
    let mut out = [0u8; KEY_LEN];
    out.copy_from_slice(&x_sk);
    Ok(out)
}

/// Convenience helper for SDK usage: derive X25519 secret key bytes from a Solana Keypair.
pub fn keypair_to_x25519_secret(
    keypair: &solana_sdk::signature::Keypair,
) -> Result<[u8; KEY_LEN], EncryptionError> {
    let keypair_bytes = keypair.to_bytes();
    ed25519_secret_to_x25519(&keypair_bytes)
}

/// High-level API: encrypt for validator using sealed boxes.
pub fn encrypt_ed25519_recipient(
    plaintext: &[u8],
    recipient_ed25519_pubkey: &[u8; KEY_LEN],
) -> Result<Vec<u8>, EncryptionError> {
    init_sodium()?;
    let ed_pk = crypto_sign::PublicKey::from_bytes(recipient_ed25519_pubkey)
        .map_err(|_| EncryptionError::InvalidEd25519PublicKey)?;
    let x_pk = crypto_sign::ed25519_pk_to_curve25519(&ed_pk)
        .map_err(|_| EncryptionError::InvalidEd25519PublicKey)?;
    let x_pk = crypto_box::PublicKey::from_bytes_exact(x_pk);
    crypto_box::seal_box(plaintext, &x_pk)
        .map_err(|_| EncryptionError::SealFailed)
}

/// Decrypt sealed box bytes back to plaintext bytes.
pub fn decrypt(
    encrypted_payload: &[u8],
    recipient_x25519_pubkey: &[u8; KEY_LEN],
    recipient_x25519_secret: &[u8; KEY_LEN],
) -> Result<Vec<u8>, EncryptionError> {
    init_sodium()?;
    let pk = crypto_box::PublicKey::from_bytes_exact(*recipient_x25519_pubkey);
    let sk = crypto_box::SecretKey::from_bytes_exact(*recipient_x25519_secret);
    crypto_box::open_sealed_box(encrypted_payload, &pk, &sk)
        .map_err(|_| EncryptionError::OpenFailed)
}

#[cfg(test)]
mod tests {
    use solana_sdk::signer::Signer;

    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let validator = solana_sdk::signature::Keypair::new();
        let validator_x25519_secret =
            keypair_to_x25519_secret(&validator).unwrap();
        let validator_x25519_pubkey =
            ed25519_pubkey_to_x25519(validator.pubkey().as_array()).unwrap();
        let plaintext = b"hello compact actions";

        let encrypted =
            encrypt_ed25519_recipient(plaintext, validator.pubkey().as_array())
                .unwrap();
        let decrypted = decrypt(
            &encrypted,
            &validator_x25519_pubkey,
            &validator_x25519_secret,
        )
        .unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_random_ephemeral_changes_ciphertext() {
        let validator = solana_sdk::signature::Keypair::new();
        let plaintext = b"same bytes";

        let c1 =
            encrypt_ed25519_recipient(plaintext, validator.pubkey().as_array())
                .unwrap();
        let c2 =
            encrypt_ed25519_recipient(plaintext, validator.pubkey().as_array())
                .unwrap();
        assert_ne!(c1, c2);
    }
}
