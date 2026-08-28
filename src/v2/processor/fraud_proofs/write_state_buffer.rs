use dlp_api::{
    error::DlpError,
    v2::{
        pda::{PROTOCOL_CONFIG_SEED, STATE_BUFFER_SEED},
        ProtocolConfig, StateBuffer, WriteStateBufferArgs,
        STATE_BUFFER_MAX_ACCOUNT_GROWTH, STATE_BUFFER_MAX_TOTAL_LEN,
    },
};
use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    AccountView, ProgramResult,
};
use wheels::{
    layout::{Decodable, Encodable},
    require_eq, require_eq_keys, require_le, require_n_accounts, require_ne,
    require_signer,
};

use crate::{
    processor::fast::utils::pda::{create_pda, resize_pda},
    requires::{
        is_uninitialized_account, require_initialized_pda, require_owned_pda,
        require_uninitialized_pda, StandardCtx,
    },
};

/// Write full account-state bytes into a DLP-owned v2 StateBuffer.
///
/// Accounts:
/// 0: `[signer, writable]` payer for StateBuffer rent
/// 1: `[signer]`           authority identity for this buffer
/// 2: `[writable]`         StateBuffer PDA
/// 3: `[]`                 delegated account whose bytes are being uploaded
/// 4: `[]`                 ProtocolConfig PDA
/// 5: `[]`                 system program, required by system CPI
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
        protocol_config,
        _system_program,
    ] = require_n_accounts!(accounts, 6);

    let args = WriteStateBufferArgs::decode(data)?;
    let (offset, write_end) = validate_args(&args)?;

    require_signer!(payer);
    require_signer!(authority);
    if !payer.is_writable() {
        return Err(ProgramError::Immutable);
    }
    require_owned_pda(
        delegated_account,
        &crate::fast::ID,
        "delegated account",
    )?;
    validate_protocol_config(protocol_config)?;

    let commit_id_bytes = args.commit_id().to_le_bytes();
    let state_buffer_seeds = [
        STATE_BUFFER_SEED,
        delegated_account.address().as_ref(),
        &commit_id_bytes,
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

        let initial_len = account_len_for(write_end)?;
        require_le!(
            initial_len,
            STATE_BUFFER_MAX_ACCOUNT_GROWTH,
            ProgramError::InvalidInstructionData
        );

        create_pda(
            state_buffer,
            &crate::fast::ID,
            initial_len,
            &[Signer::from(&[
                Seed::from(STATE_BUFFER_SEED),
                Seed::from(delegated_account.address().as_ref()),
                Seed::from(&commit_id_bytes),
                Seed::from(authority.address().as_ref()),
                Seed::from(&[state_buffer_bump]),
            ])],
            payer,
        )?;

        StateBuffer {
            discriminator: StateBuffer::DISCRIMINATOR,
            authority: authority.address().clone(),
            account_pubkey: delegated_account.address().clone(),
            commit_id: args.commit_id(),
            data_hash: [0; 32],
            total_len: args.total_len(),
            written_len: 0,
            finalized: false,
            _padding: [0; 7],
        }
        .encode_to(
            &mut state_buffer.try_borrow_mut()?.as_mut()
                [..StateBuffer::DATA_LEN],
        )?;
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
        payer,
        authority,
        delegated_account,
        state_buffer,
        &args,
        offset,
        write_end,
    )
}

fn validate_args(
    args: &dlp_api::v2::WriteStateBufferArgsView<'_>,
) -> Result<(usize, usize), ProgramError> {
    require_ne!(args.total_len(), 0, ProgramError::InvalidInstructionData);
    require_le!(
        args.total_len(),
        STATE_BUFFER_MAX_TOTAL_LEN,
        ProgramError::InvalidInstructionData
    );
    require_ne!(args.chunk().len(), 0, ProgramError::InvalidInstructionData);

    let offset = args.offset() as usize;
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

fn validate_protocol_config(protocol_config: &AccountView) -> ProgramResult {
    require_initialized_pda(
        protocol_config,
        &[PROTOCOL_CONFIG_SEED],
        &crate::fast::ID,
        false,
        "protocol config",
    )?;

    let protocol_config_data = protocol_config.try_borrow()?;
    let protocol_config_state =
        ProtocolConfig::decode(protocol_config_data.as_ref())?;
    if protocol_config_state.discriminator() != ProtocolConfig::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq!(
        protocol_config_state.paused(),
        false,
        ProgramError::InvalidAccountData
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_chunk(
    payer: &AccountView,
    authority: &AccountView,
    delegated_account: &AccountView,
    state_buffer_account: &AccountView,
    args: &dlp_api::v2::WriteStateBufferArgsView<'_>,
    offset: usize,
    write_end: usize,
) -> ProgramResult {
    let mut state = load_state_buffer(state_buffer_account)?;
    validate_state_buffer(&state, authority, delegated_account, args)?;

    if offset < state.written_len as usize {
        return validate_duplicate_chunk(
            state_buffer_account,
            args.chunk(),
            offset,
            write_end,
            state.written_len as usize,
        );
    }

    if state.finalized {
        return Err(ProgramError::InvalidInstructionData);
    }
    require_eq!(
        offset,
        state.written_len as usize,
        ProgramError::InvalidInstructionData
    );

    let required_len = account_len_for(write_end)?;
    if required_len > state_buffer_account.data_len() {
        let growth = required_len
            .checked_sub(state_buffer_account.data_len())
            .ok_or(DlpError::Overflow)?;
        require_le!(
            growth,
            STATE_BUFFER_MAX_ACCOUNT_GROWTH,
            ProgramError::InvalidInstructionData
        );
        resize_pda(payer, state_buffer_account, required_len)?;
    }

    let data_start = StateBuffer::DATA_LEN
        .checked_add(offset)
        .ok_or(DlpError::Overflow)?;
    let data_end = StateBuffer::DATA_LEN
        .checked_add(write_end)
        .ok_or(DlpError::Overflow)?;
    let total_data_end = StateBuffer::DATA_LEN
        .checked_add(state.total_len as usize)
        .ok_or(DlpError::Overflow)?;

    let mut account_data = state_buffer_account.try_borrow_mut()?;
    if account_data.len() < data_end {
        return Err(ProgramError::InvalidAccountData);
    }

    account_data.as_mut()[data_start..data_end].copy_from_slice(args.chunk());
    state.written_len = write_end as u32;

    if state.written_len == state.total_len {
        if account_data.len() < total_data_end {
            return Err(ProgramError::InvalidAccountData);
        }
        state.data_hash = account_data_hash(
            &account_data.as_ref()[StateBuffer::DATA_LEN..total_data_end],
        );
        state.finalized = true;
    }

    state.encode_to(&mut account_data.as_mut()[..StateBuffer::DATA_LEN])?;

    Ok(())
}

fn validate_state_buffer(
    state: &StateBuffer,
    authority: &AccountView,
    delegated_account: &AccountView,
    args: &dlp_api::v2::WriteStateBufferArgsView<'_>,
) -> ProgramResult {
    if state.discriminator != StateBuffer::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq_keys!(
        &state.authority,
        authority.address(),
        DlpError::InvalidAuthority
    );
    require_eq_keys!(
        &state.account_pubkey,
        delegated_account.address(),
        ProgramError::InvalidAccountData
    );
    require_eq!(
        state.commit_id,
        args.commit_id(),
        ProgramError::InvalidInstructionData
    );
    require_eq!(
        state.total_len,
        args.total_len(),
        ProgramError::InvalidInstructionData
    );

    Ok(())
}

fn validate_duplicate_chunk(
    state_buffer_account: &AccountView,
    chunk: &[u8],
    offset: usize,
    write_end: usize,
    written_len: usize,
) -> ProgramResult {
    require_le!(write_end, written_len, ProgramError::InvalidInstructionData);

    let data_start = StateBuffer::DATA_LEN
        .checked_add(offset)
        .ok_or(DlpError::Overflow)?;
    let data_end = StateBuffer::DATA_LEN
        .checked_add(write_end)
        .ok_or(DlpError::Overflow)?;

    let account_data = state_buffer_account.try_borrow()?;
    if account_data.len() < data_end {
        return Err(ProgramError::InvalidAccountData);
    }
    require_eq!(
        &account_data.as_ref()[data_start..data_end],
        chunk,
        ProgramError::InvalidInstructionData
    );

    Ok(())
}

fn load_state_buffer(
    state_buffer_account: &AccountView,
) -> Result<StateBuffer, ProgramError> {
    let account_data = state_buffer_account.try_borrow()?;
    if account_data.len() < StateBuffer::DATA_LEN {
        return Err(ProgramError::InvalidAccountData);
    }

    let state =
        StateBuffer::decode(&account_data.as_ref()[..StateBuffer::DATA_LEN])?;

    Ok(StateBuffer {
        discriminator: state.discriminator(),
        authority: *state.authority(),
        account_pubkey: *state.account_pubkey(),
        commit_id: state.commit_id(),
        data_hash: *state.data_hash(),
        total_len: state.total_len(),
        written_len: state.written_len(),
        finalized: state.finalized(),
        _padding: [0; 7],
    })
}

fn account_len_for(raw_len: usize) -> Result<usize, ProgramError> {
    StateBuffer::DATA_LEN
        .checked_add(raw_len)
        .ok_or(DlpError::Overflow.into())
}

fn account_data_hash(data: &[u8]) -> [u8; 32] {
    solana_sha256_hasher::hashv(&[b"magicblock.account_data.v1", data])
        .to_bytes()
}
