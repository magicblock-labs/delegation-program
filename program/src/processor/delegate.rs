use delegation_program_interface as dpi;
use dpi::{
    error::DlpError,
    legacy::pda::DELEGATE_BUFFER_TAG,
    pda::{DELEGATION_CLUSTER_SEED, DELEGATION_STATE_SEED},
    DelegateArgs, DelegationCluster, DelegationState, UndelegationRequester,
};
use pinocchio::{
    address::{address_eq, Address},
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    AccountView, ProgramResult,
};
use pinocchio_system::instructions as system;
use wheels::{
    layout::Decodable, require, require_eq, require_eq_keys, require_le,
    require_n_accounts, require_owned_by, require_signer,
};

use crate::{
    legacy::processor::{
        fast::utils::pda::{create_pda, AccountSpace},
        utils::curve::is_on_curve_fast,
    },
    requires::{require_uninitialized_account, StandardCtx},
};

/// Delegate an account to one delegation cluster.
///
/// Accounts:
/// 0: `[signer, writable]` delegation-state rent payer
/// 1: `[signer, writable]` delegated account, signed by its owner program
/// 2: `[]`                 original owner program
/// 3: `[writable]`         owner-program delegate buffer PDA
/// 4: `[writable]`         DelegationState PDA
/// 5: `[]`                 DelegationCluster PDA
/// 6: `[]`                 system program, required by system CPI
#[inline(never)]
pub fn process_delegate(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        payer, // force multi-line
        delegated_account,
        original_owner,
        delegate_buffer,
        delegation_state,
        delegation_cluster,
        _system_program,
    ] = require_n_accounts!(accounts, 7);

    require_signer!(payer);
    require_signer!(delegated_account);
    require_owned_by!(delegated_account, &crate::fast::ID);

    let args = DelegateArgs::decode(data)?;
    validate_delegated_account(delegated_account, original_owner, &args)?;

    let delegate_buffer_bump = [args.delegate_buffer_bump()];
    dpi::require_pda!(
        delegate_buffer,
        &[
            DELEGATE_BUFFER_TAG,
            delegated_account.address().as_ref(),
            &delegate_buffer_bump,
        ],
        original_owner.address(),
        true
    );

    let delegation_state_bump = [args.delegation_state_bump()];
    dpi::require_pda!(
        delegation_state,
        &[
            DELEGATION_STATE_SEED,
            delegated_account.address().as_ref(),
            &delegation_state_bump,
        ],
        &crate::fast::ID,
        true
    );
    require_uninitialized_account(
        delegation_state,
        true,
        StandardCtx::new("delegation state"),
    )?;

    let delegation_cluster_data = delegation_cluster.try_borrow()?;
    let delegation_cluster_state =
        DelegationCluster::decode(delegation_cluster_data.as_ref())?;
    let delegation_cluster_bump = [delegation_cluster_state.bump()];
    dpi::require_initialized_pda!(
        delegation_cluster,
        &[
            DELEGATION_CLUSTER_SEED,
            delegation_cluster_state.cluster_id().as_ref(),
            &delegation_cluster_bump,
        ],
        &crate::fast::ID,
        false
    );
    require_eq!(
        &delegation_cluster_state.discriminator(),
        &DelegationCluster::DISCRIMINATOR,
        ProgramError::InvalidAccountData
    );
    require!(
        !delegation_cluster_state.paused(),
        ProgramError::InvalidAccountData
    );
    drop(delegation_cluster_data);

    let seeds = args.seeds();
    let encoded_seeds = seeds.as_bytes();
    let state_len =
        DelegationState::data_len_from_encoded_seeds_len(encoded_seeds.len())?;
    let delegated_account_lamports = delegated_account.lamports();

    create_pda(
        delegation_state,
        &crate::fast::ID,
        AccountSpace::CurrentRent(state_len),
        &[Signer::from(&[
            Seed::from(DELEGATION_STATE_SEED),
            Seed::from(delegated_account.address().as_ref()),
            Seed::from(&delegation_state_bump),
        ])],
        payer,
    )?;

    let mut state = DelegationState::decode_mut(delegation_state)?;
    *state.discriminator_mut()? = DelegationState::DISCRIMINATOR;
    *state.bump_mut()? = args.delegation_state_bump();
    *state.delegated_account_mut()? = *delegated_account.address();
    *state.delegation_cluster_mut()? = *delegation_cluster.address();
    *state.original_owner_mut()? = *original_owner.address();
    *state.rent_payer_mut()? = *payer.address();
    state.delegation_slot_mut()?.set(Clock::get()?.slot)?;
    state.lamports_mut()?.set(delegated_account_lamports)?;
    state.last_commit_seq_mut()?.set(0)?;
    *state.undelegation_requester_mut()? = UndelegationRequester::None.value();
    *state.delegated_account_bump_mut()? = args.delegated_account_bump();
    *state.seed_count_mut()? = seeds.len() as u8;
    state
        .encoded_seeds_mut()?
        .extend_from_slice(encoded_seeds)?;

    if !delegate_buffer.is_data_empty() {
        require_eq!(
            delegate_buffer.data_len(),
            delegated_account.data_len(),
            ProgramError::InvalidAccountData
        );
        delegated_account
            .try_borrow_mut()?
            .copy_from_slice(delegate_buffer.try_borrow()?.as_ref());
    }

    // Preserve the original zero-lamport state in DelegationState, then fund
    // an empty delegated account enough to keep it alive on the base layer.
    if delegated_account_lamports == 0 && delegated_account.is_data_empty() {
        system::Transfer {
            from: payer,
            to: delegated_account,
            lamports: Rent::get()?.try_minimum_balance(0)?,
        }
        .invoke()?;
    }

    Ok(())
}

fn validate_delegated_account(
    delegated_account: &AccountView,
    original_owner: &AccountView,
    args: &dpi::DelegateArgsView<'_>,
) -> ProgramResult {
    let seeds = args.seeds();
    require_le!(
        seeds.len(),
        DelegationState::MAX_SEEDS,
        DlpError::TooManySeeds
    );

    if is_on_curve_fast(delegated_account.address()) {
        require_eq_keys!(
            original_owner.address(),
            &pinocchio_system::ID,
            ProgramError::IncorrectProgramId
        );
        require_eq!(seeds.len(), 0, ProgramError::InvalidInstructionData);
        require_eq!(
            args.delegated_account_bump(),
            0,
            ProgramError::InvalidInstructionData
        );
        return Ok(());
    }

    require!(!seeds.is_empty(), ProgramError::InvalidInstructionData);

    let mut pda_seeds: [&[u8]; DelegationState::MAX_SEEDS + 1] =
        [&[]; DelegationState::MAX_SEEDS + 1];
    for (index, seed) in seeds.iter().enumerate() {
        require_le!(
            seed.as_slice().len(),
            DelegationState::MAX_SEED_LEN,
            ProgramError::MaxSeedLengthExceeded
        );
        pda_seeds[index] = seed.as_slice();
    }
    let delegated_account_bump = [args.delegated_account_bump()];
    pda_seeds[seeds.len()] = &delegated_account_bump;

    let owner = if address_eq(original_owner.address(), &pinocchio_system::ID) {
        &crate::fast::ID
    } else {
        original_owner.address()
    };
    let derived_account =
        Address::create_program_address(&pda_seeds[..=seeds.len()], owner)
            .map_err(|_| ProgramError::InvalidSeeds)?;
    require_eq_keys!(
        &derived_account,
        delegated_account.address(),
        ProgramError::InvalidSeeds
    );

    Ok(())
}
