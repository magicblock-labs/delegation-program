use borsh::to_vec;
use dlp::{
    args::DelegateArgs,
    discriminator::DlpDiscriminator,
    pda::{
        delegate_buffer_pda_from_delegated_account_and_owner_program,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account,
    },
    total_size_budget, AccountSizeClass, DLP_PROGRAM_DATA_SIZE_CLASS,
};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};

/// Builds a delegate instruction
/// See [dlp::processor::process_delegate] for docs.
pub fn delegate(
    payer: Pubkey,
    delegated_account: Pubkey,
    owner: Option<Pubkey>,
    args: DelegateArgs,
) -> Instruction {
    build_delegate_instruction(
        DlpDiscriminator::Delegate,
        payer,
        delegated_account,
        owner,
        args,
    )
}

/// Builds a delegate instruction that allows any validator identity.
/// See [dlp::processor::process_delegate_with_any_validator] for docs.
pub fn delegate_with_any_validator(
    payer: Pubkey,
    delegated_account: Pubkey,
    owner: Option<Pubkey>,
    args: DelegateArgs,
) -> Instruction {
    build_delegate_instruction(
        DlpDiscriminator::DelegateWithAnyValidator,
        payer,
        delegated_account,
        owner,
        args,
    )
}

fn build_delegate_instruction(
    discriminator: DlpDiscriminator,
    payer: Pubkey,
    delegated_account: Pubkey,
    owner: Option<Pubkey>,
    args: DelegateArgs,
) -> Instruction {
    let owner = owner.unwrap_or(system_program::id());
    let delegate_buffer_pda =
        delegate_buffer_pda_from_delegated_account_and_owner_program(
            &delegated_account,
            &owner,
        );
    let delegation_record_pda =
        delegation_record_pda_from_delegated_account(&delegated_account);
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&delegated_account);
    let mut data = discriminator.to_vec();
    data.extend_from_slice(&to_vec(&args).unwrap());

    Instruction {
        program_id: dlp::id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(delegated_account, true),
            AccountMeta::new_readonly(owner, false),
            AccountMeta::new(delegate_buffer_pda, false),
            AccountMeta::new(delegation_record_pda, false),
            AccountMeta::new(delegation_metadata_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    }
}

///
/// Returns accounts-data-size budget for delegate instruction.
///
/// This value can be used with ComputeBudgetInstruction::SetLoadedAccountsDataSizeLimit
///
pub fn delegate_size_budget(delegated_account: AccountSizeClass) -> u32 {
    total_size_budget(&[
        DLP_PROGRAM_DATA_SIZE_CLASS,
        AccountSizeClass::Tiny, // payer
        delegated_account,      // delegated_account
        AccountSizeClass::Tiny, // owner
        delegated_account,      // delegate_buffer_pda
        AccountSizeClass::Tiny, // delegation_record_pda
        AccountSizeClass::Tiny, // delegation_metadata_pda
        AccountSizeClass::Tiny, // system_program
    ])
}
