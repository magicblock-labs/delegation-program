use dlp::{
    discriminator::DlpDiscriminator,
    pda::{
        delegate_buffer_pda_from_delegated_account_and_owner_program,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
        magic_fee_vault_pda_from_validator,
        validator_fees_vault_pda_from_validator,
    },
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};

/// Delegates the magic fee vault PDA for a validator.
/// See [crate::processor::process_delegate_magic_fee_vault] for docs.
pub fn delegate_magic_fee_vault(
    payer: Pubkey,
    validator_identity: Pubkey,
) -> Instruction {
    let validator_fees_vault =
        validator_fees_vault_pda_from_validator(&validator_identity);
    let magic_fee_vault =
        magic_fee_vault_pda_from_validator(&validator_identity);
    let delegate_buffer =
        delegate_buffer_pda_from_delegated_account_and_owner_program(
            &magic_fee_vault,
            &dlp::id(),
        );
    let delegation_record =
        delegation_record_pda_from_delegated_account(&magic_fee_vault);
    let delegation_metadata =
        delegation_metadata_pda_from_delegated_account(&magic_fee_vault);

    Instruction {
        program_id: dlp::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(validator_identity, true),
            AccountMeta::new_readonly(validator_fees_vault, false),
            AccountMeta::new(magic_fee_vault, false),
            AccountMeta::new(delegate_buffer, false),
            AccountMeta::new(delegation_record, false),
            AccountMeta::new(delegation_metadata, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(dlp::id(), false),
        ],
        data: DlpDiscriminator::DelegateMagicFeeVault.to_vec(),
    }
}
