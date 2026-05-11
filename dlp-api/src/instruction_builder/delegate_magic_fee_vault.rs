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
};
use solana_sdk_ids::system_program;

use crate::compat::{Compatize, Modernize};

/// Delegates the magic fee vault PDA for a validator.
/// See [crate::processor::process_delegate_magic_fee_vault] for docs.
pub fn delegate_magic_fee_vault(
    payer: Pubkey,
    validator_identity: Pubkey,
) -> Instruction {
    let validator_identity_compat = validator_identity.compatize();
    let validator_fees_vault =
        validator_fees_vault_pda_from_validator(&validator_identity_compat);
    let magic_fee_vault =
        magic_fee_vault_pda_from_validator(&validator_identity_compat);
    let dlp_id_compat = dlp::id().compatize();
    let delegate_buffer =
        delegate_buffer_pda_from_delegated_account_and_owner_program(
            &magic_fee_vault,
            &dlp_id_compat,
        );
    let delegation_record =
        delegation_record_pda_from_delegated_account(&magic_fee_vault);
    let delegation_metadata =
        delegation_metadata_pda_from_delegated_account(&magic_fee_vault);

    Instruction {
        program_id: dlp::id().modernize(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(validator_identity, true),
            AccountMeta::new_readonly(validator_fees_vault.modernize(), false),
            AccountMeta::new(magic_fee_vault.modernize(), false),
            AccountMeta::new(delegate_buffer.modernize(), false),
            AccountMeta::new(delegation_record.modernize(), false),
            AccountMeta::new(delegation_metadata.modernize(), false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(dlp::id().modernize(), false),
        ],
        data: DlpDiscriminator::DelegateMagicFeeVault.to_vec(),
    }
}
