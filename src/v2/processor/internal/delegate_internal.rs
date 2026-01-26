use bytemuck::{Pod, Zeroable};
use pinocchio::{
    address::address_eq,
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, Sysvar},
    AccountView, Address, ProgramResult,
};
use pinocchio_log::log;
use pinocchio_system::instructions as system;
use solana_address::PDA_MARKER;

use crate::{
    args::ArgsWithBuffer,
    consts::RENT_EXCEPTION_ZERO_BYTES_LAMPORTS,
    error::DlpError,
    pda::{self},
    pod_view::PodView,
    processor::{fast::utils::pda::create_pda, utils::curve::is_on_curve_fast},
    v2::{DelegationStateHeader, ValidatedDelegationBindings},
    v2_require_eq_keys, v2_require_n_accounts, v2_require_owned_by,
    v2_require_pda_fast, v2_require_signer, v2_require_uninitialized_pda,
    validator_fees_vault_seeds_from_validator,
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DelegateArgsHeader {
    /// The validator authority that is added to the delegation record
    pub validator: Address,

    /// The frequency at which the validator should commit the account data
    /// if no commit is triggered by the owning program
    pub commit_frequency_ms: u32,

    /// The seeds used to derive the PDA of the delegated account
    delegate_buffer_bump: u8,
    delegation_state_bump: u8,
    validator_fees_vault_bump: u8,

    reserved_padding0: [u8; 1],
    //pub seeds: Vec<Vec<u8>>,
}

// buffer contains the seeds
type DelegateArgs<'a> = ArgsWithBuffer<'a, DelegateArgsHeader>;

pub(crate) fn process_delegate_internal<
    const ALLOW_SYSTEM_PROGRAM_VALIDATOR: bool,
>(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        payer, // force multi-line
        delegated_account,
        owner_program,
        delegate_buffer_account,
        delegation_state,
        _system_program
    ] = v2_require_n_accounts!(accounts, 6);

    v2_require_owned_by!(delegated_account, &crate::fast::ID);

    // Check that payer and delegated_account are signers, this ensures the instruction is being called from CPI
    v2_require_signer!(payer);
    v2_require_signer!(delegated_account);

    let args = DelegateArgs::from_bytes(data)?;

    // Check that the buffer PDA is initialized and derived correctly from the PDA
    v2_require_pda_fast!(
        delegate_buffer_account,
        &[
            pda::DELEGATE_BUFFER_TAG,
            delegated_account.address().as_ref(),
            &[args.delegate_buffer_bump],
            owner_program.address().as_ref(),
            PDA_MARKER,
        ],
        true
    );

    v2_require_uninitialized_pda!(
        delegation_state,
        &[
            DelegationStateHeader::SEED,
            delegated_account.address().as_ref(),
            &[args.delegation_state_bump],
        ]
    );

    if !ALLOW_SYSTEM_PROGRAM_VALIDATOR {
        if args.validator.to_bytes() == pinocchio_system::ID.to_bytes() {
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
        let seeds_to_validate: &[&[u8]] = &[];
        // let seeds_to_validate: &[&[u8]] = match args.seeds.len() {
        //     1 => &[&args.seeds[0]],
        //     2 => &[&args.seeds[0], &args.seeds[1]],
        //     3 => &[&args.seeds[0], &args.seeds[1], &args.seeds[2]],
        //     4 => &[
        //         &args.seeds[0],
        //         &args.seeds[1],
        //         &args.seeds[2],
        //         &args.seeds[3],
        //     ],
        //     5 => &[
        //         &args.seeds[0],
        //         &args.seeds[1],
        //         &args.seeds[2],
        //         &args.seeds[3],
        //         &args.seeds[4],
        //     ],
        //     6 => &[
        //         &args.seeds[0],
        //         &args.seeds[1],
        //         &args.seeds[2],
        //         &args.seeds[3],
        //         &args.seeds[4],
        //         &args.seeds[5],
        //     ],
        //     7 => &[
        //         &args.seeds[0],
        //         &args.seeds[1],
        //         &args.seeds[2],
        //         &args.seeds[3],
        //         &args.seeds[4],
        //         &args.seeds[5],
        //         &args.seeds[6],
        //     ],
        //     8 => &[
        //         &args.seeds[0],
        //         &args.seeds[1],
        //         &args.seeds[2],
        //         &args.seeds[3],
        //         &args.seeds[4],
        //         &args.seeds[5],
        //         &args.seeds[6],
        //         &args.seeds[7],
        //     ],
        //     _ => return Err(DlpError::TooManySeeds.into()),
        // };
        let derived_pda =
            Address::find_program_address(seeds_to_validate, program_id).0;

        v2_require_eq_keys!(
            &derived_pda,
            delegated_account.address(),
            ProgramError::InvalidSeeds
        );
    }

    create_pda(
        delegation_state,
        &crate::fast::ID,
        DelegationStateHeader::SPACE + args.buffer.len(),
        &[Signer::from(&[
            Seed::from(DelegationStateHeader::SEED),
            Seed::from(delegated_account.address().as_ref()),
            Seed::from(&[args.delegation_state_bump]),
        ])],
        payer,
    )?;

    let mut delegation_state_data = delegation_state.try_borrow_mut()?;
    //let mut delegation_state_view =
    //    DelegationState::from_bytes(&mut delegation_state_data)?;

    let validator_fees_vault = Address::find_program_address(
        validator_fees_vault_seeds_from_validator!(args.validator),
        &crate::fast::ID,
    )
    .0;

    // Initialize the delegation record
    let header = DelegationStateHeader {
        discriminator: DelegationStateHeader::DISCRIMINATOR,
        original_owner: owner_program.address().to_bytes().into(),
        delegation_slot: Clock::get()?.slot,
        original_lamports: delegated_account.lamports(),
        commit_frequency_ms: args.commit_frequency_ms as u64,
        bindings: ValidatedDelegationBindings {
            delegated_account: *delegated_account.address(),
            validator_as_authority: args.validator,
            validator_fees_vault,
        },
        last_commit_id: 0,
        rent_payer: payer.address().to_bytes().into(),
        is_undelegatable: false.into(),
        reserved_padding0: Default::default(),
    };

    header.try_copy_to(
        &mut delegation_state_data.as_mut()[..DelegationStateHeader::SPACE],
    )?;

    // let delegation_metadata = DelegationMetadata {
    //     seeds: args.seeds,
    // };

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
