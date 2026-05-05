mod backward_compat {
    pub use borsh_compat as borsh;

    pub use solana_pubkey_compat::Pubkey;
}

mod current {
    pub use crate::solana_program::pubkey::Pubkey;
    pub use borsh_current as borsh;
}

pub use backward_compat::*;
