#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(clippy::empty_line_after_doc_comments)]

///
/// compat.rs is a boundary layer for the public API
/// ================================================
///
/// It lets dlp-api expose either the legacy compatibility types or the current
/// Solana 3.0 types at the API surface, while keeping the implementation itself
/// always on Solana 3.0.
///
/// In practice, compat::{Pubkey, borsh, ...} is used only for public function
/// parameters and return types. As soon as execution enters a function body, inputs
/// are normalized to the Solana 3.0 types, and the internal logic runs entirely on
/// Solana 3.0. If a value needs to cross back out through the public API, it is
/// converted back at the boundary.
///

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
