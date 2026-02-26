#[cfg(feature = "sdk")]
use curve25519_dalek::{
    edwards::CompressedEdwardsY, montgomery::MontgomeryPoint,
};
use serde::{Deserialize, Serialize};
use solana_program::hash::hashv;

#[cfg(feature = "sdk")]
use sha2::{Digest, Sha512};
#[cfg(feature = "sdk")]
use x25519_dalek::{PublicKey as X25519Public, StaticSecret as X25519Secret};

pub const KEY_LEN: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum EncryptionError {
    #[error("invalid ed25519 public key for x25519 conversion")]
    InvalidEd25519PublicKey,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EncryptedPayloadV1 {
    /// Ephemeral X25519 public key created by the sender for this payload.
    pub ephemeral_pubkey: [u8; KEY_LEN],

    /// Encrypted compact-action bytes.
    pub ciphertext: Vec<u8>,
}

/// Convert an Ed25519 public key into an X25519 public key.
#[cfg(feature = "sdk")]
pub fn ed25519_pubkey_to_x25519(
    ed25519_pubkey: &[u8; KEY_LEN],
) -> Option<[u8; KEY_LEN]> {
    let edwards = CompressedEdwardsY(*ed25519_pubkey).decompress()?;
    let montgomery: MontgomeryPoint = edwards.to_montgomery();
    Some(montgomery.to_bytes())
}

/// Convert an Ed25519 secret key seed into an X25519 secret key.
///
/// This follows the libsodium-style conversion:
/// SHA-512(seed) then clamp the first 32 bytes.
#[cfg(feature = "sdk")]
pub fn ed25519_secret_to_x25519(
    ed25519_secret_seed: &[u8; KEY_LEN],
) -> [u8; KEY_LEN] {
    let mut h = Sha512::new();
    h.update(ed25519_secret_seed);
    let digest = h.finalize();

    let mut out = [0u8; KEY_LEN];
    out.copy_from_slice(&digest[..KEY_LEN]);
    out[0] &= 248;
    out[31] &= 127;
    out[31] |= 64;
    out
}

/// Convenience helper for SDK usage: derive X25519 secret key bytes from a Solana Keypair.
#[cfg(feature = "sdk")]
pub fn keypair_to_x25519_secret(
    keypair: &solana_sdk::signature::Keypair,
) -> [u8; KEY_LEN] {
    let keypair_bytes = keypair.to_bytes();
    let mut seed = [0u8; KEY_LEN];
    seed.copy_from_slice(&keypair_bytes[..KEY_LEN]);
    ed25519_secret_to_x25519(&seed)
}

/// High-level API: encrypt for validator using a random ephemeral secret from OS RNG.
#[cfg(feature = "sdk")]
pub fn encrypt(
    plaintext: &[u8],
    recipient_x25519_pubkey: &[u8; KEY_LEN],
) -> Vec<u8> {
    use rand::rngs::OsRng;
    let ephemeral_secret = X25519Secret::random_from_rng(OsRng).to_bytes();
    encrypt_with_ephemeral(
        plaintext,
        recipient_x25519_pubkey,
        &ephemeral_secret,
    )
}

/// High-level API: same as above, but starts from recipient Ed25519 pubkey.
#[cfg(feature = "sdk")]
pub fn encrypt_ed25519_recipient(
    plaintext: &[u8],
    recipient_ed25519_pubkey: &[u8; KEY_LEN],
) -> Result<Vec<u8>, EncryptionError> {
    let recipient_x25519_pubkey =
        ed25519_pubkey_to_x25519(recipient_ed25519_pubkey)
            .ok_or(EncryptionError::InvalidEd25519PublicKey)?;
    Ok(encrypt(plaintext, &recipient_x25519_pubkey))
}

#[cfg(not(feature = "sdk"))]
pub fn encrypt_ed25519_recipient(
    plaintext: &[u8],
    recipient_ed25519_pubkey: &[u8; KEY_LEN],
) -> Result<Vec<u8>, EncryptionError> {
    panic!("encrypt_ed25519_recipient requires `sdk` feature");
}

/// Encrypt any plaintext bytes for a recipient X25519 public key.
///
/// The caller supplies an ephemeral secret key (random per message).
/// This function stores the ephemeral public key in the output payload.
///
/// Example:
/// `let encrypted = encrypt_with_ephemeral(b"hello", &recipient_pubkey, &ephemeral_secret);`
#[cfg(feature = "sdk")]
pub fn encrypt_with_ephemeral(
    plaintext: &[u8],
    recipient_x25519_pubkey: &[u8; KEY_LEN],
    ephemeral_x25519_secret: &[u8; KEY_LEN],
) -> Vec<u8> {
    let sender_secret = X25519Secret::from(*ephemeral_x25519_secret);
    let sender_public = X25519Public::from(&sender_secret);
    let recipient_public = X25519Public::from(*recipient_x25519_pubkey);
    let shared = sender_secret.diffie_hellman(&recipient_public).to_bytes();

    let mut ciphertext = plaintext.to_vec();
    xor_with_stream(&mut ciphertext, &shared);

    bincode::serialize(&EncryptedPayloadV1 {
        ephemeral_pubkey: sender_public.to_bytes(),
        ciphertext,
    })
    .expect("encrypted payload serialization should not fail")
}

/// Decrypt serialized encrypted payload bytes back to plaintext bytes.
///
/// Example:
/// `let plaintext = decrypt(&encrypted, &recipient_x25519_secret)?;`
#[cfg(feature = "sdk")]
pub fn decrypt(
    encrypted_payload: &[u8],
    recipient_x25519_secret: &[u8; KEY_LEN],
) -> Result<Vec<u8>, bincode::Error> {
    let EncryptedPayloadV1 {
        ephemeral_pubkey,
        mut ciphertext,
    } = bincode::deserialize(encrypted_payload)?;

    let recipient_secret = X25519Secret::from(*recipient_x25519_secret);
    let sender_public = X25519Public::from(ephemeral_pubkey);
    let shared = recipient_secret.diffie_hellman(&sender_public).to_bytes();

    xor_with_stream(&mut ciphertext, &shared);
    Ok(ciphertext)
}

fn xor_with_stream(data: &mut [u8], shared_secret: &[u8; KEY_LEN]) {
    let mut counter: u64 = 0;
    let mut offset = 0usize;

    while offset < data.len() {
        let block = hashv(&[shared_secret.as_slice(), &counter.to_le_bytes()])
            .to_bytes();

        for &k in block.iter() {
            if offset >= data.len() {
                break;
            }
            data[offset] ^= k;
            offset += 1;
        }

        counter = counter.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use x25519_dalek::{
        PublicKey as X25519Public, StaticSecret as X25519Secret,
    };

    use super::*;

    #[test]
    fn test_ed25519_secret_to_x25519_shape() {
        let seed = [7u8; KEY_LEN];
        let secret = ed25519_secret_to_x25519(&seed);
        assert_eq!(secret.len(), KEY_LEN);
        assert_eq!(secret[0] & 0b0000_0111, 0);
        assert_eq!(secret[31] & 0b1000_0000, 0);
        assert_eq!(secret[31] & 0b0100_0000, 0b0100_0000);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let validator_secret = X25519Secret::from([11u8; KEY_LEN]);
        let validator_public = X25519Public::from(&validator_secret).to_bytes();
        let validator_secret = validator_secret.to_bytes();
        let ephemeral_secret = [22u8; KEY_LEN];
        let plaintext = b"hello compact actions";

        let encrypted = encrypt_with_ephemeral(
            plaintext,
            &validator_public,
            &ephemeral_secret,
        );
        let decrypted = decrypt(&encrypted, &validator_secret).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_random_ephemeral_changes_ciphertext() {
        let validator_secret = X25519Secret::from([44u8; KEY_LEN]);
        let validator_public = X25519Public::from(&validator_secret).to_bytes();
        let plaintext = b"same bytes";

        let c1 = encrypt_with_ephemeral(
            plaintext,
            &validator_public,
            &[1u8; KEY_LEN],
        );
        let c2 = encrypt_with_ephemeral(
            plaintext,
            &validator_public,
            &[2u8; KEY_LEN],
        );
        assert_ne!(c1, c2);
    }
}
