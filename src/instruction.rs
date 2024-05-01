use crate::consts::{BUFFER, COMMIT_RECORD, DELEGATION, STATE_DIFF};
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
    CommitState = 1,
    Undelegate = 2,
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
    let delegation_pda = Pubkey::find_program_address(&[DELEGATION, &pda.to_bytes()], &crate::id());
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(pda, false),
            AccountMeta::new(owner_program, false),
            AccountMeta::new(buffer_pda.0, false),
            AccountMeta::new(delegation_pda.0, false),
            AccountMeta::new(authority, false),
            AccountMeta::new(system_program, false),
        ],
        data: [
            DlpInstruction::Delegate.to_vec(),
            DelegateArgs {
                buffer_bump: buffer_pda.1,
                authority_bump: delegation_pda.1,
            }
            .to_bytes()
            .to_vec(),
        ]
        .concat(),
    }
}

/// Builds a commit state instruction.
pub fn commit_state(
    authority: Pubkey,
    origin_account: Pubkey,
    commitment: u64,
    system_program: Pubkey,
    state: Vec<u8>,
) -> Instruction {
    let delegation_pda = Pubkey::find_program_address(&[DELEGATION, &origin_account.to_bytes()], &crate::id());
    let new_state_pda = Pubkey::find_program_address(&[STATE_DIFF, &origin_account.to_bytes()], &crate::id());
    let commit_state_record_pda = Pubkey::find_program_address(&[COMMIT_RECORD, &commitment.to_be_bytes(), &origin_account.to_bytes()], &crate::id());
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(authority, true),
            AccountMeta::new(origin_account, false),
            AccountMeta::new(new_state_pda.0, false),
            AccountMeta::new(commit_state_record_pda.0, false),
            AccountMeta::new(delegation_pda.0, false),
            AccountMeta::new(system_program, false),
        ],
        data: [
            DlpInstruction::CommitState.to_vec(),
            state,
        ].concat(),
    }
}

