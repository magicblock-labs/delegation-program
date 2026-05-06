use dlp_api::compat::borsh;
use dlp_api::require_eq;

use pinocchio::{
    address::address_eq,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    AccountView, Address, ProgramResult,
};
use pinocchio_log::log;
use pinocchio_system::instructions as system;

use crate::{
    args::DelegateWithActionsArgs,
    consts::{DEFAULT_VALIDATOR_IDENTITY, RENT_EXCEPTION_ZERO_BYTES_LAMPORTS},
    error::DlpError,
    pda,
    processor::{
        fast::{to_pinocchio_program_error, utils::pda::create_pda},
        utils::curve::is_on_curve_fast,
    },
    require_n_accounts_with_optionals,
    requires::{
        require_owned_pda, require_pda, require_signer,
        require_uninitialized_pda, DelegationMetadataCtx, DelegationRecordCtx,
    },
    state::{DelegationMetadata, DelegationRecord},
};

/// Delegates an account and stores an actions payload.
pub fn process_delegate_with_actions(
    _program_id: &Address,
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let (
        [
            payer, // force multi-line
            delegated_account,
            owner_program,
            delegate_buffer_account,
            delegation_record_account,
            delegation_metadata_account,
            _system_program,
        ],
        remaining_accounts,
    ) = require_n_accounts_with_optionals!(accounts, 7);

    require_owned_pda(
        delegated_account,
        &crate::fast::ID,
        "delegated account",
    )?;

    // Check that payer and delegated_account are signers, this ensures the instruction is being called from CPI
    require_signer(payer, "payer")?;
    require_signer(delegated_account, "delegated account")?;

    // Check that the buffer PDA is initialized and derived correctly from the PDA
    require_pda(
        delegate_buffer_account,
        &[
            pda::DELEGATE_BUFFER_TAG,
            delegated_account.address().as_ref(),
        ],
        owner_program.address(),
        true,
        "delegate buffer",
    )?;

    // Check that the delegation record PDA is uninitialized
    let delegation_record_bump = require_uninitialized_pda(
        delegation_record_account,
        &[
            pda::DELEGATION_RECORD_TAG,
            delegated_account.address().as_ref(),
        ],
        &crate::fast::ID,
        true,
        DelegationRecordCtx,
    )?;

    // Check that the delegation metadata PDA is uninitialized
    let delegation_metadata_bump = require_uninitialized_pda(
        delegation_metadata_account,
        &[
            pda::DELEGATION_METADATA_TAG,
            delegated_account.address().as_ref(),
        ],
        &crate::fast::ID,
        true,
        DelegationMetadataCtx,
    )?;

    let args: DelegateWithActionsArgs = DelegateWithActionsArgs::parse(data)?;

    // Validate instruction payload shape up-front. This confirms delegate args
    // and actions envelope format, while encrypted bytes remain opaque.
    {
        // Validate clear-text indices early when possible. Encrypted
        // AccountMetas are skipped here because they can only be validated
        // after decryption by the ER validator.
        for ix in args.actions.instructions.iter() {
            args.actions.validate_index(ix.program_id)?;

            for account in &ix.accounts {
                let crate::args::MaybeEncryptedAccountMeta::ClearText(meta) =
                    account
                else {
                    continue;
                };
                let index = args.actions.validate_index(meta.key())?;
                require_eq!(
                    meta.is_signer(),
                    args.actions.is_signer(index)?,
                    ProgramError::InvalidInstructionData
                );
            }
        }

        // Enforce required signers from the pubkey-table prefix.
        for signer in &args.actions.signers {
            let account = remaining_accounts
                .iter()
                .find(|account| {
                    address_eq(account.address(), unsafe {
                        &*(signer.as_ptr() as *const Address)
                    })
                })
                .ok_or(ProgramError::NotEnoughAccountKeys)?;
            if !account.is_signer() {
                return Err(ProgramError::MissingRequiredSignature);
            }
        }
    }

    if let Some(validator) = args.delegate.validator {
        if validator.to_bytes() == pinocchio_system::ID.to_bytes() {
            return Err(DlpError::DelegationToSystemProgramNotAllowed.into());
        }
    }

    // Validate seeds if the delegate account is not on curve, i.e. is a PDA
    // If the owner is the system program, we check if the account is derived from the delegation program,
    // allowing delegation of escrow accounts
    if !is_on_curve_fast(delegated_account.address()) {
        let program_id =
            if address_eq(owner_program.address(), &pinocchio_system::ID) {
                &crate::fast::ID
            } else {
                owner_program.address()
            };
        let seeds_to_validate: &[&[u8]] = match args.delegate.seeds.len() {
            1 => &[&args.delegate.seeds[0]],
            2 => &[&args.delegate.seeds[0], &args.delegate.seeds[1]],
            3 => &[
                &args.delegate.seeds[0],
                &args.delegate.seeds[1],
                &args.delegate.seeds[2],
            ],
            4 => &[
                &args.delegate.seeds[0],
                &args.delegate.seeds[1],
                &args.delegate.seeds[2],
                &args.delegate.seeds[3],
            ],
            5 => &[
                &args.delegate.seeds[0],
                &args.delegate.seeds[1],
                &args.delegate.seeds[2],
                &args.delegate.seeds[3],
                &args.delegate.seeds[4],
            ],
            6 => &[
                &args.delegate.seeds[0],
                &args.delegate.seeds[1],
                &args.delegate.seeds[2],
                &args.delegate.seeds[3],
                &args.delegate.seeds[4],
                &args.delegate.seeds[5],
            ],
            7 => &[
                &args.delegate.seeds[0],
                &args.delegate.seeds[1],
                &args.delegate.seeds[2],
                &args.delegate.seeds[3],
                &args.delegate.seeds[4],
                &args.delegate.seeds[5],
                &args.delegate.seeds[6],
            ],
            8 => &[
                &args.delegate.seeds[0],
                &args.delegate.seeds[1],
                &args.delegate.seeds[2],
                &args.delegate.seeds[3],
                &args.delegate.seeds[4],
                &args.delegate.seeds[5],
                &args.delegate.seeds[6],
                &args.delegate.seeds[7],
            ],
            _ => return Err(DlpError::TooManySeeds.into()),
        };
        let derived_pda =
            Address::find_program_address(seeds_to_validate, program_id).0;

        if !address_eq(&derived_pda, delegated_account.address()) {
            log!("Expected delegated PDA to be: ");
            derived_pda.log();
            log!("but got: ");
            delegated_account.address().log();
            return Err(ProgramError::InvalidSeeds);
        }
    }

    let action_data = borsh::to_vec(&args.actions)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    create_pda(
        delegation_record_account,
        &crate::fast::ID,
        DelegationRecord::size_with_discriminator() + action_data.len(),
        &[Signer::from(&[
            Seed::from(pda::DELEGATION_RECORD_TAG),
            Seed::from(delegated_account.address().as_ref()),
            Seed::from(&[delegation_record_bump]),
        ])],
        payer,
    )?;

    // Initialize the delegation record
    let delegation_record = DelegationRecord {
        owner: owner_program.address().to_bytes().into(),
        authority: args
            .delegate
            .validator
            .unwrap_or(DEFAULT_VALIDATOR_IDENTITY),
        commit_frequency_ms: args.delegate.commit_frequency_ms as u64,
        delegation_slot: Clock::get()?.slot,
        lamports: delegated_account.lamports(),
    };

    let mut delegation_record_data =
        delegation_record_account.try_borrow_mut()?;
    let record_size = DelegationRecord::size_with_discriminator();
    if delegation_record_data.len() != record_size + action_data.len() {
        return Err(DlpError::InvalidDataLength.into());
    }
    let (record_bytes, action_bytes) =
        delegation_record_data.split_at_mut(record_size);
    delegation_record
        .to_bytes_with_discriminator(record_bytes)
        .map_err(to_pinocchio_program_error)?;
    action_bytes.copy_from_slice(&action_data);

    let delegation_metadata = DelegationMetadata {
        seeds: args.delegate.seeds,
        last_update_nonce: 0,
        is_undelegatable: false,
        rent_payer: payer.address().to_bytes().into(),
    };

    // Initialize the delegation metadata PDA
    create_pda(
        delegation_metadata_account,
        &crate::fast::ID,
        delegation_metadata.serialized_size(),
        &[Signer::from(&[
            Seed::from(pda::DELEGATION_METADATA_TAG),
            Seed::from(delegated_account.address().as_ref()),
            Seed::from(&[delegation_metadata_bump]),
        ])],
        payer,
    )?;

    // Copy the seeds to the delegated metadata PDA
    let mut delegation_metadata_data =
        delegation_metadata_account.try_borrow_mut()?;
    delegation_metadata
        .to_bytes_with_discriminator(&mut delegation_metadata_data.as_mut())
        .map_err(to_pinocchio_program_error)?;

    // Copy the data from the buffer into the original account
    if !delegate_buffer_account.is_data_empty() {
        let mut delegated_data = delegated_account.try_borrow_mut()?;
        let delegate_buffer_data = delegate_buffer_account.try_borrow()?;
        (*delegated_data).copy_from_slice(&delegate_buffer_data);
    }

    // Make the account rent exempt if it's not
    if delegated_account.lamports() == 0 && delegated_account.data_len() == 0 {
        system::Transfer {
            from: payer,
            to: delegated_account,
            lamports: RENT_EXCEPTION_ZERO_BYTES_LAMPORTS,
        }
        .invoke()?;
    }

    Ok(())
}
