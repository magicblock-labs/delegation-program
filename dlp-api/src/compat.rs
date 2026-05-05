mod backward_compat {
    pub use borsh_compat as borsh;

    pub use solana_pubkey_compat::Pubkey;
}

pub mod latest {
    pub use borsh_current as borsh;
    pub use solana_program::pubkey::Pubkey;
}

pub trait Modernize {
    type Modern;
    fn modernize(self) -> Self::Modern;
}

impl Modernize for backward_compat::Pubkey {
    type Modern = latest::Pubkey;
    fn modernize(self) -> latest::Pubkey {
        self.to_bytes().into()
    }
}

#[cfg(feature = "backward-compat")]
pub use backward_compat::*;

#[cfg(not(feature = "backward-compat"))]
pub use latest::*;
