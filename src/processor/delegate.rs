use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    {self},
};

/// Delegate a Pda to an authority
///
/// 1. Copy origin to a buffer PDA
/// 2. Close origin
/// 3. Reopen origin with authority set to the delegation program
/// 4. Save new authority in the Authority Record
///
/// Accounts expected: Buffer PDA, Origin PDA, Authority Record
pub fn process_delegate<'a, 'info>(
    _program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
    data: &[u8],
) -> ProgramResult {
    // TODO: Implement delegation logic

    Ok(())
}

/// Update the data of a delegated Pda
///
/// 1. Copy delegated PDA to a buffer PDA
/// 2. Close PDA and reopen it with the origin authority
/// 3. Reopen origin with authority set to the delegation program
/// 4. Save new authority in the Authority Record
///
/// Accounts expected: Authority Record, Buffer PDA, Delegated PDA
pub fn process_update<'a, 'info>(
    _program_id: &Pubkey,
    accounts: &'a [AccountInfo<'info>],
    data: &[u8],
) -> ProgramResult {
    // TODO: Implement delegation logic

    Ok(())
}

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
