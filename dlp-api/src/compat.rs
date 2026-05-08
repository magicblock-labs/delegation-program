#![allow(unused_imports)]
#![allow(dead_code)]

mod backward_compat {
    pub use borsh_compat as borsh;
    pub use solana_pubkey_compat::{declare_id, pubkey, Pubkey};
}

pub mod latest {
    pub use borsh_current as borsh;
    pub use solana_program::{declare_id, pubkey, pubkey::Pubkey};
}

#[cfg(feature = "backward-compat")]
pub use backward_compat::*;
#[cfg(not(feature = "backward-compat"))]
pub use latest::*;

pub(crate) trait Modernize {
    type Modern;
    fn modernize(self) -> Self::Modern;
}

pub(crate) trait Compatize {
    type Compat;
    fn compatize(self) -> Self::Compat;
}

impl Modernize for backward_compat::Pubkey {
    type Modern = latest::Pubkey;
    fn modernize(self) -> latest::Pubkey {
        self.to_bytes().into()
    }
}

impl Modernize for latest::Pubkey {
    type Modern = latest::Pubkey;
    fn modernize(self) -> latest::Pubkey {
        self
    }
}

impl Compatize for backward_compat::Pubkey {
    type Compat = backward_compat::Pubkey;
    fn compatize(self) -> backward_compat::Pubkey {
        self
    }
}

impl Compatize for latest::Pubkey {
    type Compat = Pubkey;
    fn compatize(self) -> Pubkey {
        #[cfg(feature = "backward-compat")]
        {
            self.to_bytes().into()
        }

        #[cfg(not(feature = "backward-compat"))]
        {
            self
        }
    }
}

//impl Compatize for solana_address::Address {
//    type Compat = Pubkey;
//    fn compatize(self) -> Pubkey {
//        self.to_bytes().into()
//    }
//}
