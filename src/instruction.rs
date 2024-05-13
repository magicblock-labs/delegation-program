use num_enum::TryFromPrimitive;
use shank::ShankInstruction;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};

use crate::consts::{BUFFER, COMMIT_RECORD, DELEGATION, STATE_DIFF};

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, ShankInstruction, TryFromPrimitive)]
#[rustfmt::skip]
pub enum DlpInstruction {
    #[account(0, name = "payer", desc = "The fees payer", signer)]
    #[account(1, name = "delegate_account", desc = "Account to delegate", signer)]
    #[account(2, name = "owner_program", desc = "The account owner program")]
    #[account(3, name = "buffer", desc = "Buffer to hold the account data during delegation")]
    #[account(4, name = "delegation_record", desc = "The account delegation record")]
    #[account(5, name = "system_program", desc = "The system program")]
    Delegate = 0,
    #[account(0, name = "authority", desc = "The authority that commit the new sate", signer)]
    #[account(1, name = "delegated_account", desc = "The delegated account", signer)]
    #[account(2, name = "new_state", desc = "The account to store the new account state", signer)]
    #[account(3, name = "commit_state_record", desc = "Account to store the state commitment record")]
    #[account(4, name = "delegation_record", desc = "The account delegation record")]
    #[account(5, name = "system_program", desc = "The system program")]
    CommitState = 1,
    #[account(0, name = "payer", desc = "The fees payer", signer)]
    #[account(1, name = "delegated_account", desc = "The delegated account", signer)]
    #[account(2, name = "owner_program", desc = "The account owner program")]
    #[account(3, name = "buffer", desc = "Buffer to hold the account data during undelegation")]
    #[account(4, name = "new_state", desc = "The account that store the new account state", signer)]
    #[account(5, name = "committed_state_record", desc = "Account that store the state commitment record")]
    #[account(6, name = "delegation_record", desc = "The account delegation record")]
    #[account(7, name = "reimbursement", desc = "The account to reimburse the fees after closing the records accounts")]
    #[account(8, name = "system_program", desc = "The system program")]
    Undelegate = 2,
}

impl DlpInstruction {
    pub fn to_vec(&self) -> Vec<u8> {
        vec![*self as u8]
    }
}

/// Builds a delegate instruction.
pub fn delegate(
    payer: Pubkey,
    pda: Pubkey,
    owner_program: Pubkey,
    system_program: Pubkey,
    delegation_program: Pubkey,
    discriminator: Vec<u8>,
) -> Instruction {
    let buffer_pda = Pubkey::find_program_address(&[BUFFER, &pda.to_bytes()], &owner_program);
    let delegation_pda = Pubkey::find_program_address(&[DELEGATION, &pda.to_bytes()], &crate::id());
    Instruction {
        program_id: owner_program,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(pda, false),
            AccountMeta::new(owner_program, false),
            AccountMeta::new(buffer_pda.0, false),
            AccountMeta::new(delegation_pda.0, false),
            AccountMeta::new(delegation_program, false),
            AccountMeta::new(system_program, false),
        ],
        data: [discriminator].concat(),
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
    let delegation_pda =
        Pubkey::find_program_address(&[DELEGATION, &origin_account.to_bytes()], &crate::id());
    let new_state_pda =
        Pubkey::find_program_address(&[STATE_DIFF, &origin_account.to_bytes()], &crate::id());
    let commit_state_record_pda = Pubkey::find_program_address(
        &[
            COMMIT_RECORD,
            &commitment.to_be_bytes(),
            &origin_account.to_bytes(),
        ],
        &crate::id(),
    );
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
        data: [DlpInstruction::CommitState.to_vec(), state].concat(),
    }
}

/// Builds a commit state instruction.
#[allow(clippy::too_many_arguments)]
pub fn undelegate(
    payer: Pubkey,
    delegated_account: Pubkey,
    owner_program: Pubkey,
    buffer: Pubkey,
    state_diff: Pubkey,
    committed_state_record: Pubkey,
    delegation_record: Pubkey,
    reimbursement: Pubkey,
) -> Instruction {
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new(owner_program, false),
            AccountMeta::new(buffer, false),
            AccountMeta::new(state_diff, false),
            AccountMeta::new(committed_state_record, false),
            AccountMeta::new(delegation_record, false),
            AccountMeta::new(reimbursement, false),
            AccountMeta::new(system_program::id(), false),
        ],
        data: DlpInstruction::Undelegate.to_vec(),
    }
}
