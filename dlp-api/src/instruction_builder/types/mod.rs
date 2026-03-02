mod encryptable_types;

pub use encryptable_types::*;
use solana_program::pubkey::Pubkey;

pub trait Encryptable: Sized {
    type Output;
    fn encrypted(self) -> Self::Output {
        self.with_encryption(true)
    }
    fn cleartext(self) -> Self::Output {
        self.with_encryption(false)
    }
    fn with_encryption(self, encrypt: bool) -> Self::Output;
}

pub trait EncryptableFrom: Sized {
    type Output;
    fn encrypted_from(self, offset: usize) -> Self::Output;
}

pub trait Encrypt: Sized {
    type Output;
    type Error;

    fn encrypt(self, validator: &Pubkey) -> Result<Self::Output, Self::Error>;
}
