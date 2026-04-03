use crate::solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    system_program,
};

use crate::{
    args::DelegateArgs,
    discriminator::DlpDiscriminator,
    magic_fee_vault_seeds_from_validator,
    pda::magic_fee_vault_pda_from_validator,
    processor::utils::loaders::{
        load_initialized_pda, load_initialized_validator_fees_vault,
        load_program, load_signer,
    },
};

/// Delegates the magic fee vault PDA for a validator.
///
/// Accounts:
///
/// 0: `[writable, signer]` payer
/// 1: `[signer]`           validator identity
/// 2: `[]`                 validator fees vault (proves validator is registered)
/// 3: `[writable]`         magic fee vault PDA
/// 4: `[writable]`         delegate buffer PDA
/// 5: `[writable]`         delegation record PDA
/// 6: `[writable]`         delegation metadata PDA
/// 7: `[]`                 system program
/// 8: `[]`                 this program
///
/// Requirements:
///
/// - payer must be a signer
/// - validator must be a signer and registered (has a validator fees vault)
/// - magic fee vault must be the PDA derived from this validator and be initialized
/// - delegation record must be uninitialized
pub fn process_delegate_magic_fee_vault(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    let [payer, validator, validator_fees_vault, magic_fee_vault, delegate_buffer, delegation_record, delegation_metadata, system_program, delegation_program] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    load_signer(payer, "payer")?;
    load_signer(validator, "validator")?;
    load_program(system_program, system_program::id(), "system program")?;
    load_program(delegation_program, crate::id(), "delegation program")?;

    // Verify validator is registered
    load_initialized_validator_fees_vault(
        validator,
        validator_fees_vault,
        false,
    )?;

    // Verify the magic fee vault is indeed derived from this validator
    let expected_vault = magic_fee_vault_pda_from_validator(validator.key);
    if !expected_vault.eq(magic_fee_vault.key) {
        msg!(
            "Magic fee vault {} is not derived from validator {}",
            magic_fee_vault.key,
            validator.key
        );
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify magic fee vault is initialized (owned by this program)
    let magic_fee_vault_bump = load_initialized_pda(
        magic_fee_vault,
        magic_fee_vault_seeds_from_validator!(validator.key),
        &crate::id(),
        true,
        "magic fee vault",
    )?;

    let magic_fee_vault_seeds: &[&[u8]] =
        magic_fee_vault_seeds_from_validator!(validator.key);

    let delegate_args = DelegateArgs {
        commit_frequency_ms: 0,
        seeds: magic_fee_vault_seeds.iter().map(|s| s.to_vec()).collect(),
        validator: Some(*validator.key),
    };

    let mut data = DlpDiscriminator::Delegate.to_vec();
    data.extend(borsh::to_vec(&delegate_args)?);

    // Create delegation ix
    let ix = Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*magic_fee_vault.key, true),
            AccountMeta::new_readonly(crate::id(), false),
            AccountMeta::new(*delegate_buffer.key, false),
            AccountMeta::new(*delegation_record.key, false),
            AccountMeta::new(*delegation_metadata.key, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    };

    let magic_fee_vault_bump_slice = &[magic_fee_vault_bump];
    let magic_fee_vault_signer_seeds =
        [magic_fee_vault_seeds, &[magic_fee_vault_bump_slice]].concat();

    invoke_signed(
        &ix,
        &[
            delegation_program.clone(),
            payer.clone(),
            magic_fee_vault.clone(),
            delegate_buffer.clone(),
            delegation_record.clone(),
            delegation_metadata.clone(),
            system_program.clone(),
        ],
        &[&magic_fee_vault_signer_seeds],
    )?;

    Ok(())
}
