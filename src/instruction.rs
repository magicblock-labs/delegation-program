use num_enum::TryFromPrimitive;
use shank::ShankInstruction;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, ShankInstruction, TryFromPrimitive)]
#[rustfmt::skip]
pub enum DlpInstruction {

    #[account(0, name = "origin", desc = "Origin authority", signer)]
    #[account(1, name = "authority", desc = "Delegate authority")]
    #[account(2, name = "buffer", desc = "Data buffer")]
    Delegate = 0,
}

impl DlpInstruction {
    pub fn to_vec(&self) -> Vec<u8> {
        vec![*self as u8]
    }
}

/// Builds a delegate instruction.
pub fn delegate(origin: Pubkey, authority: Pubkey, buffer: Pubkey) -> Instruction {
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(origin, true),
            AccountMeta::new(authority, false),
            AccountMeta::new(buffer, false),
        ],
        data: [DlpInstruction::Delegate.to_vec()].concat(),
    }
}
