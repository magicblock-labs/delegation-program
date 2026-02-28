use dlp::{
    consts::DELEGATION_PROGRAM_DATA_ID,
    discriminator::DlpDiscriminator,
    pda::{
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        undelegate_buffer_pda_from_delegated_account,
    },
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};

/// Builds an admin-only undelegate instruction for confined accounts.
/// See [dlp::processor::process_undelegate_confined_account] for docs.
pub fn undelegate_confined_account(
    admin: Pubkey,
    delegated_account: Pubkey,
    owner_program: Pubkey,
) -> Instruction {
    let undelegate_buffer_pda =
        undelegate_buffer_pda_from_delegated_account(&delegated_account);
    let delegation_record_pda =
        delegation_record_pda_from_delegated_account(&delegated_account);
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&delegated_account);
    let delegation_program_data = DELEGATION_PROGRAM_DATA_ID.to_bytes().into();

    Instruction {
        program_id: dlp::id(),
        accounts: vec![
            AccountMeta::new(admin, true),
            AccountMeta::new(delegated_account, false),
            AccountMeta::new_readonly(owner_program, false),
            AccountMeta::new(undelegate_buffer_pda, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(delegation_program_data, false),
        ],
        data: DlpDiscriminator::UndelegateConfinedAccount.to_vec(),
    }
}
