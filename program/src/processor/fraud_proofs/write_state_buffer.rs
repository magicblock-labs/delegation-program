use delegation_program_interface as dpi;
use dpi::{
    error::DlpError, pda::STATE_BUFFER_SEED, StateBuffer, WriteStateBufferArgs,
};
use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{rent::Rent, Sysvar},
    AccountView, ProgramResult,
};
use wheels::{
    layout::{Decodable, Encodable, MaxLenStorage},
    require_eq, require_eq_keys, require_le, require_n_accounts, require_ne,
    require_owned_by, require_signer,
};

use crate::{
    legacy::processor::fast::utils::pda::create_pda_with_rent_exempt_lamports,
    processor::state_buffer_payload_hash,
    requires::{
        is_uninitialized_account, require_initialized_pda,
        require_uninitialized_pda, StandardCtx,
    },
};

/// Write payload bytes into a DLP-owned v2 StateBuffer.
///
/// Accounts:
/// 0: `[signer, writable]` payer for StateBuffer rent
/// 1: `[signer]`           authority identity for this buffer
/// 2: `[writable]`         StateBuffer PDA
/// 3: `[]`                 delegated account whose bytes are being uploaded
/// 4: `[]`                 system program, required by system CPI
#[inline(never)]
pub fn process_write_state_buffer(
    accounts: &[AccountView],
    data: &[u8],
) -> ProgramResult {
    let [
        payer, // force multi-line
        authority,
        state_buffer,
        delegated_account,
        _system_program,
    ] = require_n_accounts!(accounts, 5);

    require_signer!(payer);
    require_signer!(authority);
    require_owned_by!(delegated_account, &crate::fast::ID);

    let args = WriteStateBufferArgs::decode(data)?;
    let (offset, write_end) = validate_args(&args)?;

    let commit_seq_bytes = args.commit_seq().to_le_bytes();
    let state_buffer_seeds = [
        STATE_BUFFER_SEED,
        delegated_account.address().as_ref(),
        &commit_seq_bytes,
        authority.address().as_ref(),
    ];

    if is_uninitialized_account(state_buffer) {
        require_eq!(offset, 0, ProgramError::InvalidInstructionData);

        let state_buffer_bump = require_uninitialized_pda(
            state_buffer,
            &state_buffer_seeds,
            &crate::fast::ID,
            true,
            StandardCtx::new("state buffer"),
        )?;

        let initial_payload_capacity = (args.total_len() as usize)
            .min(StateBuffer::MAX_INITIAL_PAYLOAD_LEN);

        require_le!(
            write_end,
            initial_payload_capacity,
            ProgramError::InvalidInstructionData
        );

        let initial_len = StateBuffer::data_len_from_payload_capacity(
            initial_payload_capacity,
        )?;
        let final_len = StateBuffer::data_len_from_payload_capacity(
            args.total_len() as usize,
        )?;

        // Note that rent_exempt_lamports is computed using final_len, not initial_len,
        // because later writes can then avoid rent calculation and top-up transfer.
        let rent_exempt_lamports =
            Rent::get()?.try_minimum_balance(final_len)?;

        create_pda_with_rent_exempt_lamports(
            state_buffer,
            &crate::fast::ID,
            initial_len,
            rent_exempt_lamports,
            &[Signer::from(&[
                Seed::from(STATE_BUFFER_SEED),
                Seed::from(delegated_account.address().as_ref()),
                Seed::from(&commit_seq_bytes),
                Seed::from(authority.address().as_ref()),
                Seed::from(&[state_buffer_bump]),
            ])],
            payer,
        )?;

        StateBuffer {
            discriminator: StateBuffer::DISCRIMINATOR,
            bump: state_buffer_bump,
            authority: *authority.address(),
            account_pubkey: *delegated_account.address(),
            commit_seq: args.commit_seq(),
            data_hash: [0; 32],
            total_len: args.total_len(),
            finalized: false,
            payload_alignment_padding: [0; 2],
            payload: Vec::new(),
        }
        .encode_to(state_buffer.try_borrow_mut()?.as_mut())?;
    } else {
        require_initialized_pda(
            state_buffer,
            &state_buffer_seeds,
            &crate::fast::ID,
            true,
            "state buffer",
        )?;
    }

    write_chunk(
        authority,
        delegated_account,
        state_buffer,
        &args,
        offset,
        write_end,
    )
}

fn validate_args(
    args: &dpi::WriteStateBufferArgsView<'_>,
) -> Result<(usize, usize), ProgramError> {
    require_le!(
        args.total_len(),
        StateBuffer::MAX_TOTAL_PAYLOAD_LEN,
        ProgramError::InvalidInstructionData
    );
    let offset = args.offset() as usize;

    // An account with no data can still carry lamports and require a commitment.
    if args.total_len() == 0 {
        require_eq!(
            args.chunk().len(),
            0,
            ProgramError::InvalidInstructionData
        );
        require_eq!(offset, 0, ProgramError::InvalidInstructionData);
    } else {
        require_ne!(
            args.chunk().len(),
            0,
            ProgramError::InvalidInstructionData
        );
    }

    let write_end = offset
        .checked_add(args.chunk().len())
        .ok_or(DlpError::Overflow)?;

    require_le!(
        write_end,
        args.total_len() as usize,
        ProgramError::InvalidInstructionData
    );

    Ok((offset, write_end))
}

fn write_chunk(
    authority: &AccountView,
    delegated_account: &AccountView,
    state_buffer_account: &AccountView,
    args: &dpi::WriteStateBufferArgsView<'_>,
    offset: usize,
    write_end: usize,
) -> ProgramResult {
    let state_fields = validate_state_buffer(
        state_buffer_account,
        authority,
        delegated_account,
        args,
    )?;

    // Treat exact overlap as an idempotent retry when the prior write landed.
    if offset < state_fields.payload_len {
        return validate_duplicate_chunk(
            state_buffer_account,
            args.chunk(),
            offset,
            write_end,
            state_fields.payload_len,
        );
    }

    require_eq!(
        state_fields.finalized,
        false,
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        offset,
        state_fields.payload_len,
        ProgramError::InvalidInstructionData
    );

    StateBuffer::decode_mut(&MaxLenStorage::new(
        state_buffer_account,
        StateBuffer::data_len_from_payload_capacity(state_fields.total_len)?,
        StateBuffer::MAX_ACCOUNT_DATA_GROWTH_PER_WRITE,
    ))?
    .payload_mut()?
    .extend_from_slice(args.chunk())?;

    let data_hash = if write_end == state_fields.total_len {
        let account_data = state_buffer_account.try_borrow()?;
        let state = StateBuffer::decode(account_data.as_ref())?;
        let payload = state.payload();
        require_eq!(
            payload.len(),
            state.total_len() as usize,
            ProgramError::InvalidAccountData
        );
        Some(state_buffer_payload_hash(payload.as_slice()))
    } else {
        None
    };

    if let Some(data_hash) = data_hash {
        let mut state_mut = StateBuffer::decode_mut(state_buffer_account)?;
        *state_mut.data_hash_mut()? = data_hash;
        state_mut.finalized_mut()?.set(true)?;
    }

    Ok(())
}

struct StateBufferFields {
    total_len: usize,
    payload_len: usize,
    finalized: bool,
}

fn validate_state_buffer(
    state_buffer_account: &AccountView,
    authority: &AccountView,
    delegated_account: &AccountView,
    args: &dpi::WriteStateBufferArgsView<'_>,
) -> Result<StateBufferFields, ProgramError> {
    let account_data = state_buffer_account.try_borrow()?;
    let state = StateBuffer::decode(account_data.as_ref())?;

    require_eq!(
        &state.discriminator(),
        &StateBuffer::DISCRIMINATOR,
        ProgramError::InvalidAccountData
    );
    require_eq_keys!(
        state.authority(),
        authority.address(),
        DlpError::InvalidAuthority
    );
    require_eq_keys!(
        state.account_pubkey(),
        delegated_account.address(),
        ProgramError::InvalidAccountData
    );
    require_eq!(
        state.commit_seq(),
        args.commit_seq(),
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        state.total_len(),
        args.total_len(),
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        &state.payload_alignment_padding(),
        &[0; 2],
        ProgramError::InvalidAccountData
    );
    require_le!(
        state.payload().len(),
        state.total_len() as usize,
        ProgramError::InvalidAccountData
    );
    require_le!(
        state.payload().capacity(),
        state.total_len() as usize,
        ProgramError::InvalidAccountData
    );
    require_eq!(
        account_data.len(),
        StateBuffer::data_len_from_payload_capacity(
            state.payload().capacity()
        )?,
        ProgramError::InvalidAccountData
    );

    Ok(StateBufferFields {
        total_len: state.total_len() as usize,
        payload_len: state.payload().len(),
        finalized: state.finalized(),
    })
}

fn validate_duplicate_chunk(
    state_buffer_account: &AccountView,
    chunk: &[u8],
    offset: usize,
    write_end: usize,
    payload_len: usize,
) -> ProgramResult {
    require_le!(write_end, payload_len, ProgramError::InvalidInstructionData);

    let account_data = state_buffer_account.try_borrow()?;
    let state = StateBuffer::decode(account_data.as_ref())?;
    let payload = state.payload();
    require_eq!(
        &payload.as_slice()[offset..write_end],
        chunk,
        ProgramError::InvalidInstructionData
    );

    Ok(())
}
