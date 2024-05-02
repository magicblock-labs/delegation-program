use solana_program::{
    {self},
    account_info::AccountInfo
    ,
    entrypoint::ProgramResult
    , pubkey::Pubkey,
};
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program::invoke;

/// Undelegate a delegated Pda
///
/// 1. Copy origin to a buffer PDA
/// 2. Close origin and reopen it with authority set to the delegation program
/// 3. Copy buffer PDA to the origin PDA and close the Authority Record
///
/// Accounts expected: Authority Record, Buffer PDA, Delegated PDA
pub fn process_undelegate<'a, 'info>(
    _program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
    data: &[u8],
) -> ProgramResult {
    // TODO: Implement delegation logic

    Ok(())
}

const CLOSE_INSTRUCTION_DISCRIMINATOR: [u8; 8] = [98, 165, 201, 177, 108, 65, 206, 96];

fn call_close_pda<'a, 'info>(
    account_to_close: &'a AccountInfo<'info>,
    destination_account: &'a AccountInfo<'info>,
    program_id: &Pubkey, // Anchor program's ID
) -> ProgramResult {
    let instruction_data = CLOSE_INSTRUCTION_DISCRIMINATOR.to_vec();

    let close_instruction = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta {
                pubkey: *account_to_close.key,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: *account_to_close.key,
                is_signer: false,
                is_writable: true,
            },
        ],
        data: instruction_data,
    };

    invoke(
        &close_instruction,
        &[account_to_close.clone(), destination_account.clone()],
    )
}
