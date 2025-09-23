use borsh::BorshDeserialize;
use light_sdk::account::LightAccount;
use light_sdk::address::v1::derive_address;
use light_sdk::cpi::{CpiAccounts, CpiInputs};
use solana_program::msg;
use solana_program::program_error::ProgramError;
use solana_program::sysvar::Sysvar;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, pubkey::Pubkey, system_program,
};

use crate::args::DelegateCompressedArgs;
use crate::consts::DEFAULT_VALIDATOR_IDENTITY;
use crate::processor::utils::curve::is_on_curve;
use crate::processor::utils::loaders::{load_signer, load_uninitialized_pda};
use crate::state::DelegatedCompressedAccount;
use crate::LIGHT_CPI_SIGNER;

/// Delegates an account
///
/// Accounts:
/// 0: `[signer]`   the account paying for the transaction
/// 1: `[signer]`   the account to delegate
/// 2: `[]`         the owner of the account to delegate
///
/// Requirements:
///
/// - delegated account is uninitialized
///
/// Steps:
/// 1. Checks that the account is owned by the delegation program, that the account is initialized and derived correctly from the PDA
///  - Also checks that the delegated_account is a signer (enforcing that the instruction is being called from CPI) & other constraints
/// 2. Copies the data from the account into the delegated account
/// 3. Creates a Delegated Account to store useful information about the delegation event
/// 4. Creates a Delegated Account Seeds to store the seeds used to derive the delegate account. Needed for undelegation.
/// 5. Calls the light system program to create the account
///
/// Usage:
///
/// This instruction is meant to be called via CPI with the owning program signing for the
/// delegated account.
pub fn process_delegate_compressed(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let [payer, delegated_account, owner_program, remaining_accounts @ ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let args = DelegateCompressedArgs::try_from_slice(data)?;

    // Ensure the account does not exist to avoid conflict with its compressed account
    let seeds = args
        .seeds
        .iter()
        .map(|v| v.as_slice())
        .collect::<Vec<&[u8]>>();
    load_uninitialized_pda(
        delegated_account,
        &seeds,
        &crate::id(),
        false,
        "delegated account",
    )?;
    load_signer(payer, "payer")?;

    msg!("Delegating: {}", delegated_account.key);

    // Validate seeds if the delegate account is not on curve, i.e. is a PDA
    // If the owner is the system program, we check if the account is derived from the delegation program,
    // allowing delegation of escrow accounts
    if !is_on_curve(delegated_account.key) {
        let seeds_to_validate: Vec<&[u8]> = args.seeds.iter().map(|v| v.as_slice()).collect();
        let program_id = if owner_program.key.eq(&system_program::id()) {
            crate::id()
        } else {
            *owner_program.key
        };
        let (derived_pda, _) =
            Pubkey::find_program_address(seeds_to_validate.as_ref(), &program_id);

        if derived_pda.ne(delegated_account.key) {
            msg!(
                "Expected delegated PDA to be {}, but got {}",
                derived_pda,
                delegated_account.key
            );
            return Err(ProgramError::InvalidSeeds);
        }
    }

    let light_cpi_accounts = CpiAccounts::new(payer, &remaining_accounts, LIGHT_CPI_SIGNER);

    let (address, address_seed) = derive_address(
        &seeds,
        &args
            .address_tree_info
            .get_tree_pubkey(&light_cpi_accounts)
            .map_err(|_| ProgramError::NotEnoughAccountKeys)?,
        &crate::id(),
    );

    let new_address_params = args
        .address_tree_info
        .into_new_address_params_packed(address_seed);

    let mut delegated_account_data = LightAccount::<'_, DelegatedCompressedAccount>::new_init(
        &crate::ID,
        Some(address),
        args.account_meta.output_state_tree_index,
    );
    delegated_account_data.owner = *payer.key;
    delegated_account_data.authority = args.validator.unwrap_or(DEFAULT_VALIDATOR_IDENTITY);
    delegated_account_data.seeds = args.seeds;
    delegated_account_data.account_data = args.account_data;
    delegated_account_data.commit_frequency_ms = args.commit_frequency_ms as u64;
    delegated_account_data.delegation_slot = solana_program::clock::Clock::get()?.slot;

    let cpi = CpiInputs::new_with_address(
        args.proof,
        vec![delegated_account_data
            .to_account_info()
            .map_err(ProgramError::from)?],
        vec![new_address_params],
    );
    cpi.invoke_light_system_program(light_cpi_accounts)
        .map_err(ProgramError::from)?;

    Ok(())
}
