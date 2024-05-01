use crate::consts::{BUFFER, DELEGATION};
use crate::{impl_instruction_from_bytes, impl_to_bytes};
use bytemuck::{Pod, Zeroable};
use num_enum::TryFromPrimitive;
use shank::ShankInstruction;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

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
    #[account(0, name = "payer", desc = "The fees payer", signer)]
    #[account(1, name = "pda", desc = "Account to delegate", signer)]
    #[account(2, name = "owner_program", desc = "The pda's owner")]
    #[account(3, name = "buffer", desc = "Data buffer")]
    #[account(4, name = "delegation_record", desc = "The delegation record PDA")]
    #[account(5, name = "authority", desc = "Delegate authority", signer)]
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
pub fn delegate(
    payer: Pubkey,
    pda: Pubkey,
    owner_program: Pubkey,
    authority: Pubkey,
    system_program: Pubkey,
) -> Instruction {
    let buffer_pda = Pubkey::find_program_address(&[BUFFER, &pda.to_bytes()], &crate::id());
    let authority_pda = Pubkey::find_program_address(&[DELEGATION, &pda.to_bytes()], &crate::id());
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(pda, false), // TODO: set to true, to check this was called from the owner program
            AccountMeta::new(owner_program, false),
            AccountMeta::new(buffer_pda.0, false),
            AccountMeta::new(authority_pda.0, false),
            AccountMeta::new(authority, false),
            AccountMeta::new(system_program, false),
        ],
        data: [
            DlpInstruction::Delegate.to_vec(),
            DelegateArgs {
                buffer_bump: buffer_pda.1,
                authority_bump: authority_pda.1,
            }
            .to_bytes()
            .to_vec(),
        ]
        .concat(),
    }
}
