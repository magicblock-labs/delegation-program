use serde::{Deserialize, Serialize};

use crate::{args::MaybeEncryptedInstruction, compact, compact::ClearText};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Instruction {
    pub program_id: u8,
    pub accounts: Vec<compact::AccountMeta>,
    pub data: Vec<u8>,
}

impl Instruction {
    pub fn from_instruction(
        ix: solana_program::instruction::Instruction,
        index_of: &mut impl FnMut(
            /*account_key*/ solana_program::pubkey::Pubkey,
            /*signer*/ bool,
        ) -> u8,
    ) -> Instruction {
        Instruction {
            program_id: index_of(ix.program_id, false),

            accounts: ix
                .accounts
                .iter()
                .map(|meta| {
                    compact::AccountMeta::try_new(
                        index_of(meta.pubkey, meta.is_signer),
                        meta.is_signer,
                        meta.is_writable,
                    )
                    .expect("compact account index must fit in 6 bits")
                })
                .collect(),

            data: ix.data,
        }
    }
}

impl ClearText for Instruction {
    type Output = MaybeEncryptedInstruction;

    fn cleartext(self) -> Self::Output {
        MaybeEncryptedInstruction {
            program_id: self.program_id,
            accounts: self
                .accounts
                .into_iter()
                .map(|meta| meta.cleartext())
                .collect(),
            data: self.data.cleartext(),
        }
    }
}
