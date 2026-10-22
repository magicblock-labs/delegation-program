use delegation_program_interface as dpi;
use dpi::{consts::EXTERNAL_UNDELEGATE_DISCRIMINATOR, UNDELEGATE_BUFFER_SEED};
use pinocchio::{
    cpi::{invoke_signed, Seed, Signer},
    error::ProgramError,
    instruction::{InstructionAccount, InstructionView},
    AccountView, ProgramResult,
};

use crate::{
    legacy::processor::fast::utils::pda::{
        close_pda, create_pda, AccountSpace,
    },
    require_eq_keys,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn return_account_to_owner(
    payer: &AccountView,
    delegated_account: &AccountView,
    owner_program: &AccountView,
    undelegate_buffer: &AccountView,
    system_program: &AccountView,
    undelegate_buffer_bump: u8,
    callback_data: &[u8],
) -> ProgramResult {
    let account_lamports = delegated_account.lamports();

    if delegated_account.is_data_empty() {
        unsafe {
            delegated_account.assign(owner_program.address());
        }
        return Ok(());
    }

    let bump = [undelegate_buffer_bump];
    let signer_seeds = [
        Seed::from(UNDELEGATE_BUFFER_SEED),
        Seed::from(delegated_account.address().as_ref()),
        Seed::from(&bump),
    ];
    let signers = [Signer::from(&signer_seeds)];

    create_pda(
        undelegate_buffer,
        &crate::fast::ID,
        AccountSpace::CurrentRent(delegated_account.data_len()),
        &signers,
        payer,
    )?;

    undelegate_buffer
        .try_borrow_mut()?
        .copy_from_slice(delegated_account.try_borrow()?.as_ref());
    delegated_account.try_borrow_mut()?.fill(0);
    unsafe {
        delegated_account.assign(owner_program.address());
    }

    invoke_signed(
        &InstructionView {
            program_id: owner_program.address(),
            data: callback_data,
            accounts: &[
                InstructionAccount::new(
                    delegated_account.address(),
                    true,
                    false,
                ),
                InstructionAccount::new(
                    undelegate_buffer.address(),
                    true,
                    true,
                ),
                InstructionAccount::new(payer.address(), true, true),
                InstructionAccount::new(system_program.address(), false, false),
            ],
        },
        &[delegated_account, undelegate_buffer, payer, system_program],
        &signers,
    )?;

    require_eq_keys!(
        unsafe { delegated_account.owner() },
        owner_program.address(),
        ProgramError::InvalidAccountOwner
    );
    if delegated_account.lamports() != account_lamports {
        return Err(
            dpi::error::DlpError::InvalidValidatorBalanceAfterCPI.into()
        );
    }
    if delegated_account.try_borrow()?.as_ref()
        != undelegate_buffer.try_borrow()?.as_ref()
    {
        return Err(dpi::error::DlpError::InvalidAccountDataAfterCPI.into());
    }

    close_pda(undelegate_buffer, payer)
}

pub(crate) fn owner_callback_data(
    state: &dpi::DelegationStateView<'_>,
) -> Result<Vec<u8>, ProgramError> {
    let seeds = state
        .seeds()
        .map_err(|_| ProgramError::InvalidAccountData)?;
    let capacity = EXTERNAL_UNDELEGATE_DISCRIMINATOR
        .len()
        .checked_add(4)
        .and_then(|len| len.checked_add(state.encoded_seeds().len()))
        .and_then(|len| len.checked_add(seeds.len().saturating_mul(3)))
        .ok_or(dpi::error::DlpError::Overflow)?;
    let mut data = Vec::with_capacity(capacity);
    data.extend_from_slice(&EXTERNAL_UNDELEGATE_DISCRIMINATOR);
    data.extend_from_slice(
        &u32::try_from(seeds.len())
            .map_err(|_| dpi::error::DlpError::Overflow)?
            .to_le_bytes(),
    );
    for seed in seeds.iter() {
        data.extend_from_slice(
            &u32::try_from(seed.as_slice().len())
                .map_err(|_| dpi::error::DlpError::Overflow)?
                .to_le_bytes(),
        );
        data.extend_from_slice(seed.as_slice());
    }

    Ok(data)
}
