use num_enum::TryFromPrimitive;
use bytemuck::{Pod, Zeroable};
use shank::ShankInstruction;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use crate::{impl_instruction_from_bytes, impl_to_bytes};
use crate::consts::{AUTHORITY, BUFFER};

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct DelegateArgs {
    pub buffer_bump: u8,
    pub authority_bump: u8,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, ShankInstruction, TryFromPrimitive)]
#[rustfmt::skip]
pub enum DlpInstruction {
    #[account(0, name = "pda", desc = "Pda to delegate", signer)]
    #[account(1, name = "owner_program", desc = "The pda's owner", signer)]
    #[account(2, name = "buffer", desc = "Data buffer")]
    #[account(3, name = "authority_record", desc = "The pda's owner", signer)]
    #[account(4, name = "authority", desc = "Delegate authority")]
    Delegate = 0,
}

impl DlpInstruction {
    pub fn to_vec(&self) -> Vec<u8> {
        vec![*self as u8]
    }
}

impl_to_bytes!(DelegateArgs);
impl_instruction_from_bytes!(DelegateArgs);

/// Builds a delegate instruction.
pub fn delegate(pda: Pubkey, owner_program: Pubkey, authority: Pubkey, system_program: Pubkey) -> Instruction {
    let buffer_pda = Pubkey::find_program_address(&[BUFFER, &pda.to_bytes()], &crate::id());
    let authority_pda = Pubkey::find_program_address(&[AUTHORITY, &pda.to_bytes()], &crate::id());
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(pda, false),
            AccountMeta::new(owner_program, false),
            AccountMeta::new(buffer_pda.0, false),
            AccountMeta::new(authority_pda.0, false),
            AccountMeta::new(authority, true),
            AccountMeta::new(system_program, false),
        ],
        data: [
            DlpInstruction::Delegate.to_vec(),
            DelegateArgs {
                buffer_bump: buffer_pda.1,
                authority_bump: authority_pda.1
            }.to_bytes().to_vec(),
        ].concat(),
    }
}
