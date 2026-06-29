use dlp::solana_program;
use dlp_api::{
    args::{CommitFinalizeArgs, CommitStateArgs, DelegateArgs},
    pda::{
        commit_record_pda_from_delegated_account,
        commit_state_pda_from_delegated_account,
        delegation_metadata_pda_from_delegated_account,
        delegation_record_pda_from_delegated_account, fees_vault_pda,
        validator_fees_vault_pda_from_validator,
    },
    state::{CommitRecord, DelegationMetadata, DelegationRecord},
};
use solana_program::{
    hash::Hash, native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, rent::Rent,
};
use solana_program_test::{read_file, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction as system_instruction;

use crate::fixtures::{
    create_delegation_record_data, get_delegation_metadata_data,
    get_delegation_metadata_data_on_curve, COMMIT_NEW_STATE_ACCOUNT_DATA,
    DELEGATED_PDA_ID, DELEGATED_PDA_OWNER_ID, ON_CURVE_KEYPAIR, TEST_AUTHORITY,
};

mod fixtures;

#[tokio::test]
async fn test_commit_finalize_system_account_after_balance_decrease() {
    test_commit_system_account_after_balance_decrease(false, false).await;
}

#[tokio::test]
async fn test_commit_undelegate_system_account_after_balance_decrease() {
    test_commit_system_account_after_balance_decrease(true, false).await;
}

#[tokio::test]
async fn test_commit_finalize_pda_after_balance_decrease() {
    test_commit_system_account_after_balance_decrease(false, true).await;
}

#[tokio::test]
async fn test_commit_undelegate_pda_after_balance_decrease() {
    test_commit_system_account_after_balance_decrease(true, true).await;
}

#[tokio::test]
async fn test_commit_finalize_system_account_after_balance_increase() {
    test_commit_system_account_after_balance_increase(false, false).await;
}

#[tokio::test]
async fn test_commit_undelegate_system_account_after_balance_increase() {
    test_commit_system_account_after_balance_increase(true, false).await;
}

#[tokio::test]
async fn test_commit_finalize_pda_after_balance_increase() {
    test_commit_system_account_after_balance_increase(false, true).await;
}

#[tokio::test]
async fn test_commit_undelegate_pda_after_balance_increase() {
    test_commit_system_account_after_balance_increase(true, true).await;
}

async fn get_delegation_record(
    banks: &BanksClient,
    delegated_account: Pubkey,
) -> DelegationRecord {
    let acc = banks
        .get_account(delegation_record_pda_from_delegated_account(
            &delegated_account,
        ))
        .await
        .unwrap()
        .unwrap();
    *DelegationRecord::try_from_bytes_with_discriminator(&acc.data).unwrap()
}

async fn validator_fees_vault_balance(
    banks: &BanksClient,
    validator: Pubkey,
) -> u64 {
    banks
        .get_balance(validator_fees_vault_pda_from_validator(&validator))
        .await
        .unwrap()
}

#[tokio::test]
async fn test_commit_finalize_lamports_settlement() {
    let initial_lamports = 1_000_000;
    let (base_banks, payer, delegated, validator, blockhash) =
        setup_program_for_delegate_base_increase(initial_lamports).await;

    // Assign delegated account to the delegation program.
    let assign_ix =
        system_instruction::assign(&delegated.pubkey(), &dlp_api::id());
    let assign_tx = Transaction::new_signed_with_payer(
        &[assign_ix],
        Some(&payer.pubkey()),
        &[&payer, &delegated],
        blockhash,
    );
    base_banks.process_transaction(assign_tx).await.unwrap();

    assert_eq!(
        base_banks.get_balance(delegated.pubkey()).await.unwrap(),
        initial_lamports
    );

    // let's predent that lamports_on_ephem is the valid that tracks the current lamports
    // on the ER
    let mut lamports_on_ephem = initial_lamports;

    // Delegate the account.
    {
        let delegate_ix = dlp_api::instruction_builder::delegate(
            payer.pubkey(),
            delegated.pubkey(),
            None,
            DelegateArgs {
                commit_frequency_ms: u32::MAX,
                seeds: vec![],
                validator: Some(validator.pubkey()),
            },
        );
        let delegate_tx = Transaction::new_signed_with_payer(
            &[delegate_ix],
            Some(&payer.pubkey()),
            &[&payer, &delegated],
            blockhash,
        );
        base_banks.process_transaction(delegate_tx).await.unwrap();

        assert_eq!(
            get_delegation_record(&base_banks, delegated.pubkey())
                .await
                .lamports,
            initial_lamports,
            "delegation_record.lamports == delegated_account.lamports() at the time of delegation"
        );
    }

    // send 100 lamports to the delegated account on the base
    {
        let transfer_ix = system_instruction::transfer(
            &payer.pubkey(),
            &delegated.pubkey(),
            100,
        );
        let transfer_tx = Transaction::new_signed_with_payer(
            &[transfer_ix],
            Some(&payer.pubkey()),
            &[&payer],
            blockhash,
        );
        base_banks.process_transaction(transfer_tx).await.unwrap();

        assert_eq!(
            base_banks.get_balance(delegated.pubkey()).await.unwrap(),
            initial_lamports + 100
        );
    }

    // first commit (assume there is no lamports change on ER)
    {
        let mut args = CommitFinalizeArgs {
            commit_id: 1,
            lamports: lamports_on_ephem,
            allow_undelegation: false.into(),
            data_is_diff: false.into(),
            bumps: Default::default(),
            reserved_padding: Default::default(),
        };
        let (commit_ix, _) = dlp_api::instruction_builder::commit_finalize(
            validator.pubkey(),
            delegated.pubkey(),
            system_program::id(),
            payer.pubkey(),
            &mut args,
            &[],
        );
        let commit_tx = Transaction::new_signed_with_payer(
            &[commit_ix],
            Some(&payer.pubkey()),
            &[&validator, &payer],
            blockhash,
        );
        base_banks.process_transaction(commit_tx).await.unwrap();

        assert_eq!(
            base_banks.get_balance(validator.pubkey()).await.unwrap(),
            LAMPORTS_PER_SOL,
            "there must not be any change in validator lamports because there is no tx"
        );
        assert_eq!(
            validator_fees_vault_balance(&base_banks, validator.pubkey()).await,
            dlp_api::consts::RENT_EXCEPTION_ZERO_BYTES_LAMPORTS,
            "there must not be any change in fees_vault lamports because there is no tx"
        );

        assert_eq!(
            get_delegation_record(&base_banks, delegated.pubkey())
                .await
                .lamports,
            lamports_on_ephem,
            "delegation_record.lamports must be same as the lamports on ER (commit_lamports)"
        );

        assert_eq!(
            base_banks.get_balance(delegated.pubkey()).await.unwrap(),
            initial_lamports + 100,
            "account's lamports on the base must be unchanged"
        );
    }

    // second commit (still no lamports change on ER)
    {
        let mut args = CommitFinalizeArgs {
            commit_id: 2,
            lamports: lamports_on_ephem,
            allow_undelegation: false.into(),
            data_is_diff: false.into(),
            bumps: Default::default(),
            reserved_padding: Default::default(),
        };
        let (commit_ix, _) = dlp_api::instruction_builder::commit_finalize(
            validator.pubkey(),
            delegated.pubkey(),
            system_program::id(),
            payer.pubkey(),
            &mut args,
            &[],
        );
        let commit_tx = Transaction::new_signed_with_payer(
            &[commit_ix],
            Some(&payer.pubkey()),
            &[&validator, &payer],
            blockhash,
        );
        base_banks.process_transaction(commit_tx).await.unwrap();

        assert_eq!(
            base_banks.get_balance(validator.pubkey()).await.unwrap(),
            LAMPORTS_PER_SOL,
            "there must not be any change in validator lamports because there is no tx"
        );
        assert_eq!(
            validator_fees_vault_balance(&base_banks, validator.pubkey()).await,
            dlp_api::consts::RENT_EXCEPTION_ZERO_BYTES_LAMPORTS,
            "there must not be any change in fees_vault lamports because there is no tx"
        );

        assert_eq!(
            get_delegation_record(&base_banks, delegated.pubkey())
                .await
                .lamports,
            lamports_on_ephem,
            "delegation_record.lamports must be same as the lamports on ER (commit_lamports)"
        );

        assert_eq!(
            base_banks.get_balance(delegated.pubkey()).await.unwrap(),
            initial_lamports + 100,
            "account's lamports on the base must be unchanged"
        );
    }

    // third commit (lamports on ER has increased by 959)
    {
        // pretend lamports has increased on the ER
        lamports_on_ephem += 959;

        let mut args = CommitFinalizeArgs {
            commit_id: 3,
            lamports: lamports_on_ephem, // it has increased 959
            allow_undelegation: false.into(),
            data_is_diff: false.into(),
            bumps: Default::default(),
            reserved_padding: Default::default(),
        };
        let (commit_ix, _) = dlp_api::instruction_builder::commit_finalize(
            validator.pubkey(),
            delegated.pubkey(),
            system_program::id(),
            payer.pubkey(),
            &mut args,
            &[],
        );
        let commit_tx = Transaction::new_signed_with_payer(
            &[commit_ix],
            Some(&payer.pubkey()),
            &[&validator, &payer],
            blockhash,
        );
        base_banks.process_transaction(commit_tx).await.unwrap();

        assert_eq!(
            base_banks.get_balance(validator.pubkey()).await.unwrap(),
            LAMPORTS_PER_SOL - 959,
            "validator's lamports must decrease by 959 because 959 must be transferred to delegated_account"
        );
        assert_eq!(
            validator_fees_vault_balance(&base_banks, validator.pubkey()).await,
            dlp_api::consts::RENT_EXCEPTION_ZERO_BYTES_LAMPORTS,
            "there must not be any change in fees_vault lamports because tx deals with increased lamports value"
        );

        assert_eq!(
            get_delegation_record(&base_banks, delegated.pubkey())
                .await
                .lamports,
            lamports_on_ephem,
            "delegation_record.lamports must have increased by 100, but still same as lamports_on_ephem"
        );

        assert_eq!(
            base_banks.get_balance(delegated.pubkey()).await.unwrap(),
            initial_lamports + 100 + 959,
            "account's lamports on the base must be increased by the same amount as the change on the ER"
        );
    }

    // fourth commit (lamports on ER has decreased by 9590)
    {
        // pretend lamports has decreased on the ER
        lamports_on_ephem -= 9590;

        let mut args = CommitFinalizeArgs {
            commit_id: 4,
            lamports: lamports_on_ephem, // it has increased 959
            allow_undelegation: false.into(),
            data_is_diff: false.into(),
            bumps: Default::default(),
            reserved_padding: Default::default(),
        };
        let (commit_ix, _) = dlp_api::instruction_builder::commit_finalize(
            validator.pubkey(),
            delegated.pubkey(),
            system_program::id(),
            payer.pubkey(),
            &mut args,
            &[],
        );
        let commit_tx = Transaction::new_signed_with_payer(
            &[commit_ix],
            Some(&payer.pubkey()),
            &[&validator, &payer],
            blockhash,
        );
        base_banks.process_transaction(commit_tx).await.unwrap();

        assert_eq!(
            base_banks.get_balance(validator.pubkey()).await.unwrap(),
            LAMPORTS_PER_SOL - 959,
            "validator's lamports must not changhe now"
        );
        assert_eq!(
            validator_fees_vault_balance(&base_banks, validator.pubkey()).await,
            dlp_api::consts::RENT_EXCEPTION_ZERO_BYTES_LAMPORTS + 9590,
            "validator_fees_vault_balance must have increased by 9590 now"
        );

        assert_eq!(
            get_delegation_record(&base_banks, delegated.pubkey())
                .await
                .lamports,
            lamports_on_ephem,
        );

        assert_eq!(
            base_banks.get_balance(delegated.pubkey()).await.unwrap(),
            initial_lamports + 100 + 959 - 9590,
            "account's lamports on the base must be decreased by the same amount as the change on the ER"
        );
    }
}

#[tokio::test]
async fn test_commit_and_finalize_lamports_settlement() {
    let initial_lamports = 1_000_000;
    let (mut base_banks, payer, delegated, validator, blockhash) =
        setup_program_for_delegate_base_increase(initial_lamports).await;

    // Assign delegated account to the delegation program.
    let assign_ix =
        system_instruction::assign(&delegated.pubkey(), &dlp_api::id());
    let assign_tx = Transaction::new_signed_with_payer(
        &[assign_ix],
        Some(&payer.pubkey()),
        &[&payer, &delegated],
        blockhash,
    );
    base_banks.process_transaction(assign_tx).await.unwrap();

    assert_eq!(
        base_banks.get_balance(delegated.pubkey()).await.unwrap(),
        initial_lamports
    );

    let mut lamports_on_ephem = initial_lamports;

    // Delegate the account.
    {
        let delegate_ix = dlp_api::instruction_builder::delegate(
            payer.pubkey(),
            delegated.pubkey(),
            None,
            DelegateArgs {
                commit_frequency_ms: u32::MAX,
                seeds: vec![],
                validator: Some(validator.pubkey()),
            },
        );
        let delegate_tx = Transaction::new_signed_with_payer(
            &[delegate_ix],
            Some(&payer.pubkey()),
            &[&payer, &delegated],
            blockhash,
        );
        base_banks.process_transaction(delegate_tx).await.unwrap();

        assert_eq!(
            get_delegation_record(&base_banks, delegated.pubkey())
                .await
                .lamports,
            initial_lamports,
            "delegation_record.lamports == delegated_account.lamports() at the time of delegation"
        );
    }

    // send 100 lamports to the delegated account on the base
    {
        let transfer_ix = system_instruction::transfer(
            &payer.pubkey(),
            &delegated.pubkey(),
            100,
        );
        let transfer_tx = Transaction::new_signed_with_payer(
            &[transfer_ix],
            Some(&payer.pubkey()),
            &[&payer],
            blockhash,
        );
        base_banks.process_transaction(transfer_tx).await.unwrap();

        assert_eq!(
            base_banks.get_balance(delegated.pubkey()).await.unwrap(),
            initial_lamports + 100
        );
    }

    // first commit+finalize (assume there is no lamports change on ER)
    {
        commit_state_with_nonce(CommitStateWithNonceArgs {
            banks: &mut base_banks,
            authority: &validator,
            fee_payer: &payer,
            new_delegated_account_lamports: lamports_on_ephem,
            nonce: 1,
            allow_undelegation: false,
            label: "first commit",
            delegated_account: delegated.pubkey(),
            delegated_account_owner: system_program::id(),
        })
        .await;

        finalize_with_fee_payer(FinalizeWithFeePayerArgs {
            banks: &mut base_banks,
            authority: &validator,
            fee_payer: &payer,
            label: "first finalize",
            delegated_account: delegated.pubkey(),
            owner_program: system_program::id(),
        })
        .await;

        assert_eq!(
            validator_fees_vault_balance(&base_banks, validator.pubkey()).await,
            dlp_api::consts::RENT_EXCEPTION_ZERO_BYTES_LAMPORTS,
            "there must not be any change in fees_vault lamports because there is no tx"
        );

        assert_eq!(
            get_delegation_record(&base_banks, delegated.pubkey())
                .await
                .lamports,
            lamports_on_ephem,
            "delegation_record.lamports must be same as the lamports on ER (commit_lamports)"
        );

        assert_eq!(
            base_banks.get_balance(delegated.pubkey()).await.unwrap(),
            initial_lamports + 100,
            "account's lamports on the base must be unchanged"
        );
    }

    // second commit+finalize (still no lamports change on ER)
    {
        commit_state_with_nonce(CommitStateWithNonceArgs {
            banks: &mut base_banks,
            authority: &validator,
            fee_payer: &payer,
            new_delegated_account_lamports: lamports_on_ephem,
            nonce: 2,
            allow_undelegation: false,
            label: "second commit",
            delegated_account: delegated.pubkey(),
            delegated_account_owner: system_program::id(),
        })
        .await;

        finalize_with_fee_payer(FinalizeWithFeePayerArgs {
            banks: &mut base_banks,
            authority: &validator,
            fee_payer: &payer,
            label: "second finalize",
            delegated_account: delegated.pubkey(),
            owner_program: system_program::id(),
        })
        .await;

        assert_eq!(
            validator_fees_vault_balance(&base_banks, validator.pubkey()).await,
            dlp_api::consts::RENT_EXCEPTION_ZERO_BYTES_LAMPORTS,
            "there must not be any change in fees_vault lamports because there is no tx"
        );

        assert_eq!(
            get_delegation_record(&base_banks, delegated.pubkey())
                .await
                .lamports,
            lamports_on_ephem,
            "delegation_record.lamports must be same as the lamports on ER (commit_lamports)"
        );

        assert_eq!(
            base_banks.get_balance(delegated.pubkey()).await.unwrap(),
            initial_lamports + 100,
            "account's lamports on the base must be unchanged"
        );
    }

    // third commit+finalize (lamports on ER has increased by 959)
    {
        lamports_on_ephem += 959;

        commit_state_with_nonce(CommitStateWithNonceArgs {
            banks: &mut base_banks,
            authority: &validator,
            fee_payer: &payer,
            new_delegated_account_lamports: lamports_on_ephem,
            nonce: 3,
            allow_undelegation: false,
            label: "third commit",
            delegated_account: delegated.pubkey(),
            delegated_account_owner: system_program::id(),
        })
        .await;

        finalize_with_fee_payer(FinalizeWithFeePayerArgs {
            banks: &mut base_banks,
            authority: &validator,
            fee_payer: &payer,
            label: "third finalize",
            delegated_account: delegated.pubkey(),
            owner_program: system_program::id(),
        })
        .await;

        assert_eq!(
            validator_fees_vault_balance(&base_banks, validator.pubkey()).await,
            dlp_api::consts::RENT_EXCEPTION_ZERO_BYTES_LAMPORTS,
            "there must not be any change in fees_vault lamports because tx deals with increased lamports value"
        );

        assert_eq!(
            get_delegation_record(&base_banks, delegated.pubkey())
                .await
                .lamports,
            lamports_on_ephem,
            "delegation_record.lamports must have increased by 100, but still same as lamports_on_ephem"
        );

        assert_eq!(
            base_banks.get_balance(delegated.pubkey()).await.unwrap(),
            initial_lamports + 100 + 959,
            "account's lamports on the base must be increased by the same amount as the change on the ER"
        );
    }

    // fourth commit+finalize (lamports on ER has decreased by 9590)
    {
        lamports_on_ephem -= 9590;

        commit_state_with_nonce(CommitStateWithNonceArgs {
            banks: &mut base_banks,
            authority: &validator,
            fee_payer: &payer,
            new_delegated_account_lamports: lamports_on_ephem,
            nonce: 4,
            allow_undelegation: false,
            label: "fourth commit",
            delegated_account: delegated.pubkey(),
            delegated_account_owner: system_program::id(),
        })
        .await;

        finalize_with_fee_payer(FinalizeWithFeePayerArgs {
            banks: &mut base_banks,
            authority: &validator,
            fee_payer: &payer,
            label: "fourth finalize",
            delegated_account: delegated.pubkey(),
            owner_program: system_program::id(),
        })
        .await;

        assert_eq!(
            validator_fees_vault_balance(&base_banks, validator.pubkey()).await,
            dlp_api::consts::RENT_EXCEPTION_ZERO_BYTES_LAMPORTS + 9590,
            "validator_fees_vault_balance must have increased by 9590 now"
        );

        assert_eq!(
            get_delegation_record(&base_banks, delegated.pubkey())
                .await
                .lamports,
            lamports_on_ephem,
        );

        assert_eq!(
            base_banks.get_balance(delegated.pubkey()).await.unwrap(),
            initial_lamports + 100 + 959 - 9590,
            "account's lamports on the base must be decreased by the same amount as the change on the ER"
        );
    }
}

#[tokio::test]
async fn test_commit_finalise_system_account_after_balance_decrease_and_increase_mainchain(
) {
    test_commit_system_account_after_balance_decrease_and_increase_mainchain(
        false, false,
    )
    .await;
}

#[tokio::test]
async fn test_commit_undelegate_system_account_after_balance_decrease_and_increase_mainchain(
) {
    test_commit_system_account_after_balance_decrease_and_increase_mainchain(
        true, false,
    )
    .await;
}

#[tokio::test]
async fn test_commit_finalise_pda_after_balance_decrease_and_increase_mainchain(
) {
    test_commit_system_account_after_balance_decrease_and_increase_mainchain(
        false, true,
    )
    .await;
}

#[tokio::test]
async fn test_commit_undelegate_pda_after_balance_decrease_and_increase_mainchain(
) {
    test_commit_system_account_after_balance_decrease_and_increase_mainchain(
        true, true,
    )
    .await;
}

#[tokio::test]
async fn test_commit_finalise_system_account_after_balance_increase_and_increase_mainchain(
) {
    test_commit_system_account_after_balance_increase_and_increase_mainchain(
        false, false,
    )
    .await;
}

#[tokio::test]
async fn test_commit_undelegate_system_account_after_balance_increase_and_increase_mainchain(
) {
    test_commit_system_account_after_balance_increase_and_increase_mainchain(
        true, false,
    )
    .await;
}

#[tokio::test]
async fn test_commit_finalise_pda_after_balance_increase_and_increase_mainchain(
) {
    test_commit_system_account_after_balance_increase_and_increase_mainchain(
        false, true,
    )
    .await;
}

#[tokio::test]
async fn test_commit_undelegate_pda_after_balance_increase_and_increase_mainchain(
) {
    test_commit_system_account_after_balance_increase_and_increase_mainchain(
        true, true,
    )
    .await;
}

pub async fn test_commit_system_account_after_balance_decrease(
    also_undelegate: bool,
    is_pda: bool,
) {
    // Setup
    let (delegated_account, owner_program) =
        get_delegated_account_and_owner(is_pda);
    let (mut banks, _, authority, blockhash) =
        setup_program_for_commit_test_env(SetupProgramCommitTestEnvArgs {
            delegated_account_init_lamports: LAMPORTS_PER_SOL,
            delegated_account_current_lamports: LAMPORTS_PER_SOL,
            validator_vault_init_lamports: Rent::default().minimum_balance(0),
            delegated_account,
            owner_program,
        })
        .await;

    let new_delegated_account_lamports = LAMPORTS_PER_SOL - 100;

    commit_new_state(CommitNewStateArgs {
        banks: &mut banks,
        authority: &authority,
        blockhash,
        new_delegated_account_lamports,
        delegated_account,
        delegated_account_owner: owner_program,
    })
    .await;

    finalize_and_maybe_undelegate(
        also_undelegate,
        delegated_account,
        &mut banks,
        &authority,
        blockhash,
        owner_program,
    )
    .await;

    // Assert finalized lamports balance is correct
    let delegated_account =
        banks.get_account(delegated_account).await.unwrap().unwrap();
    assert_eq!(delegated_account.lamports, new_delegated_account_lamports);

    // Assert the vault own the difference
    let validator_vault = banks
        .get_account(validator_fees_vault_pda_from_validator(
            &authority.pubkey(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(
        validator_vault.lamports >= Rent::default().minimum_balance(0) + 100
    );
}

async fn test_commit_system_account_after_balance_increase(
    also_undelegate: bool,
    is_pda: bool,
) {
    // Setup
    let (delegated_account, owner_program) =
        get_delegated_account_and_owner(is_pda);
    let (mut banks, _, authority, blockhash) =
        setup_program_for_commit_test_env(SetupProgramCommitTestEnvArgs {
            delegated_account_init_lamports: LAMPORTS_PER_SOL,
            delegated_account_current_lamports: LAMPORTS_PER_SOL,
            validator_vault_init_lamports: Rent::default().minimum_balance(0),
            delegated_account,
            owner_program,
        })
        .await;

    let new_delegated_account_lamports = LAMPORTS_PER_SOL + 100;

    commit_new_state(CommitNewStateArgs {
        banks: &mut banks,
        authority: &authority,
        blockhash,
        new_delegated_account_lamports,
        delegated_account,
        delegated_account_owner: owner_program,
    })
    .await;

    finalize_and_maybe_undelegate(
        also_undelegate,
        delegated_account,
        &mut banks,
        &authority,
        blockhash,
        owner_program,
    )
    .await;

    // Assert finalized lamports balance is correct
    let delegated_account =
        banks.get_account(delegated_account).await.unwrap().unwrap();
    assert_eq!(delegated_account.lamports, new_delegated_account_lamports);

    // Assert the vault own the difference
    let validator_vault = banks
        .get_account(validator_fees_vault_pda_from_validator(
            &authority.pubkey(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(validator_vault.lamports >= Rent::default().minimum_balance(0));
}

async fn test_commit_system_account_after_balance_decrease_and_increase_mainchain(
    also_undelegate: bool,
    is_pda: bool,
) {
    // Setup
    let (delegated_account, owner_program) =
        get_delegated_account_and_owner(is_pda);
    let (mut banks, _, authority, blockhash) =
        setup_program_for_commit_test_env(SetupProgramCommitTestEnvArgs {
            delegated_account_init_lamports: LAMPORTS_PER_SOL,
            delegated_account_current_lamports: LAMPORTS_PER_SOL + 9000, // Simulate someone transferring lamports to the delegated account
            validator_vault_init_lamports: Rent::default().minimum_balance(0),
            delegated_account,
            owner_program,
        })
        .await;

    let new_delegated_account_lamports = LAMPORTS_PER_SOL - 100;

    commit_new_state(CommitNewStateArgs {
        banks: &mut banks,
        authority: &authority,
        blockhash,
        new_delegated_account_lamports,
        delegated_account,
        delegated_account_owner: owner_program,
    })
    .await;

    finalize_and_maybe_undelegate(
        also_undelegate,
        delegated_account,
        &mut banks,
        &authority,
        blockhash,
        owner_program,
    )
    .await;

    // Assert finalized lamports balance is correct
    let delegated_account =
        banks.get_account(delegated_account).await.unwrap().unwrap();
    assert_eq!(
        delegated_account.lamports,
        new_delegated_account_lamports + 9000
    );

    // Assert the vault own the difference
    let validator_vault = banks
        .get_account(validator_fees_vault_pda_from_validator(
            &authority.pubkey(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(validator_vault.lamports >= Rent::default().minimum_balance(0));
}

async fn test_commit_system_account_after_balance_increase_and_increase_mainchain(
    also_undelegate: bool,
    is_pda: bool,
) {
    // Setup
    let (delegated_account, owner_program) =
        get_delegated_account_and_owner(is_pda);
    let (mut banks, _, authority, blockhash) =
        setup_program_for_commit_test_env(SetupProgramCommitTestEnvArgs {
            delegated_account_init_lamports: LAMPORTS_PER_SOL,
            delegated_account_current_lamports: LAMPORTS_PER_SOL + 8200, // Simulate someone transferring lamports to the delegated account
            validator_vault_init_lamports: Rent::default().minimum_balance(0),
            delegated_account,
            owner_program,
        })
        .await;

    let new_delegated_account_lamports = LAMPORTS_PER_SOL + 300;

    commit_new_state(CommitNewStateArgs {
        banks: &mut banks,
        authority: &authority,
        blockhash,
        new_delegated_account_lamports,
        delegated_account,
        delegated_account_owner: owner_program,
    })
    .await;

    finalize_and_maybe_undelegate(
        also_undelegate,
        delegated_account,
        &mut banks,
        &authority,
        blockhash,
        owner_program,
    )
    .await;

    // Assert finalized lamports balance is correct
    let delegated_account =
        banks.get_account(delegated_account).await.unwrap().unwrap();
    assert_eq!(
        delegated_account.lamports,
        new_delegated_account_lamports + 8200
    );

    // Assert the vault own the difference
    let validator_vault = banks
        .get_account(validator_fees_vault_pda_from_validator(
            &authority.pubkey(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(validator_vault.lamports >= Rent::default().minimum_balance(0));
}

fn get_delegated_account_and_owner(is_pda: bool) -> (Pubkey, Pubkey) {
    let (delegated_account, owner_program) = if is_pda {
        (DELEGATED_PDA_ID, DELEGATED_PDA_OWNER_ID)
    } else {
        (
            crate::fixtures::keypair_from_bytes(&ON_CURVE_KEYPAIR).pubkey(),
            system_program::id(),
        )
    };
    (delegated_account, owner_program)
}

async fn finalize_and_maybe_undelegate(
    also_undelegate: bool,
    delegated_account: Pubkey,
    banks: &mut BanksClient,
    authority: &Keypair,
    blockhash: Hash,
    owner_program: Pubkey,
) {
    finalize_new_state(FinalizeNewStateArgs {
        banks,
        authority,
        blockhash,
        delegated_account,
        owner_program,
    })
    .await;
    if also_undelegate {
        undelegate(UndelegateArgs {
            banks,
            authority,
            blockhash,
            delegated_account,
            owner_program,
        })
        .await;
    }
}

struct UndelegateArgs<'a> {
    banks: &'a mut BanksClient,
    authority: &'a Keypair,
    blockhash: Hash,
    delegated_account: Pubkey,
    owner_program: Pubkey,
}

async fn undelegate(args: UndelegateArgs<'_>) {
    // Retrieve the accounts
    let delegation_record_pda =
        delegation_record_pda_from_delegated_account(&args.delegated_account);

    // Submit the undelegate tx
    let ix = dlp_api::instruction_builder::undelegate(
        args.authority.pubkey(),
        args.delegated_account,
        args.owner_program,
        args.authority.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&args.authority.pubkey()),
        &[&args.authority],
        args.blockhash,
    );
    let res = args.banks.process_transaction(tx).await;
    assert!(res.is_ok(), "{:?}", res);

    // Assert the delegation_record_pda was closed
    let delegation_record_account =
        args.banks.get_account(delegation_record_pda).await.unwrap();
    assert!(delegation_record_account.is_none());

    // Assert the delegated metadata account pda was closed
    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&args.delegated_account);
    let delegation_metadata_account = args
        .banks
        .get_account(delegation_metadata_pda)
        .await
        .unwrap();
    assert!(delegation_metadata_account.is_none());

    // Assert that the account owner is now set to the original owner program
    let pda_account = args
        .banks
        .get_account(args.delegated_account)
        .await
        .unwrap()
        .unwrap();
    assert!(pda_account.owner.eq(&args.owner_program));
}

struct FinalizeNewStateArgs<'a> {
    banks: &'a mut BanksClient,
    authority: &'a Keypair,
    blockhash: Hash,
    delegated_account: Pubkey,
    owner_program: Pubkey,
}

async fn finalize_new_state(args: FinalizeNewStateArgs<'_>) {
    let ix = dlp_api::instruction_builder::finalize(
        args.authority.pubkey(),
        args.delegated_account,
        args.owner_program,
        args.authority.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&args.authority.pubkey()),
        &[&args.authority],
        args.blockhash,
    );
    let res = args.banks.process_transaction(tx).await;
    assert!(res.is_ok());

    // Assert that the account owner is still the delegation program
    let pda_account = args
        .banks
        .get_account(args.delegated_account)
        .await
        .unwrap()
        .unwrap();
    assert!(pda_account.owner.eq(&dlp_api::id()));
}

struct CommitNewStateArgs<'a> {
    banks: &'a mut BanksClient,
    authority: &'a Keypair,
    blockhash: Hash,
    new_delegated_account_lamports: u64,
    delegated_account: Pubkey,
    delegated_account_owner: Pubkey,
}

struct CommitStateWithNonceArgs<'a> {
    banks: &'a mut BanksClient,
    authority: &'a Keypair,
    fee_payer: &'a Keypair,
    new_delegated_account_lamports: u64,
    nonce: u64,
    allow_undelegation: bool,
    label: &'a str,
    delegated_account: Pubkey,
    delegated_account_owner: Pubkey,
}

async fn setup_program_for_delegate_base_increase(
    initial_lamports: u64,
) -> (BanksClient, Keypair, Keypair, Keypair, Hash) {
    assert!(
        initial_lamports >= dlp_api::consts::RENT_EXCEPTION_ZERO_BYTES_LAMPORTS,
        "Please pass lamports >= {}, but passed: {}",
        dlp_api::consts::RENT_EXCEPTION_ZERO_BYTES_LAMPORTS,
        initial_lamports
    );

    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);

    let delegated = Keypair::new();
    let validator = crate::fixtures::keypair_from_bytes(&TEST_AUTHORITY);

    program_test.add_account(
        delegated.pubkey(),
        Account {
            lamports: initial_lamports,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    program_test.add_account(
        validator.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    program_test.add_account(
        validator_fees_vault_pda_from_validator(&validator.pubkey()),
        Account {
            lamports: Rent::default().minimum_balance(0),
            data: vec![],
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;

    (banks, payer, delegated, validator, blockhash)
}

async fn commit_new_state(args: CommitNewStateArgs<'_>) {
    let data = if args.delegated_account.eq(&DELEGATED_PDA_ID) {
        COMMIT_NEW_STATE_ACCOUNT_DATA.to_vec()
    } else {
        vec![]
    };
    let commit_args = CommitStateArgs {
        data: data.clone(),
        nonce: 1,
        allow_undelegation: true,
        lamports: args.new_delegated_account_lamports,
    };

    // Commit the state for the delegated account
    let ix = dlp_api::instruction_builder::commit_state(
        args.authority.pubkey(),
        args.delegated_account,
        args.delegated_account_owner,
        commit_args,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&args.authority.pubkey()),
        &[&args.authority],
        args.blockhash,
    );
    let res = args.banks.process_transaction(tx).await;
    assert!(res.is_ok(), "{:?}", res);

    // Assert the state commitment was created and contains the new state
    let commit_state_pda =
        commit_state_pda_from_delegated_account(&args.delegated_account);
    let commit_state_account = args
        .banks
        .get_account(commit_state_pda)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(commit_state_account.data, data);

    // Check that the commit has enough collateral to finalize the proposed state diff
    let delegated_account = args
        .banks
        .get_account(args.delegated_account)
        .await
        .unwrap()
        .unwrap();
    assert!(
        args.new_delegated_account_lamports
            < commit_state_account.lamports + delegated_account.lamports
    );

    // Assert the record about the commitment exists
    let commit_record_pda =
        commit_record_pda_from_delegated_account(&args.delegated_account);
    let commit_record_account = args
        .banks
        .get_account(commit_record_pda)
        .await
        .unwrap()
        .unwrap();
    let commit_record = CommitRecord::try_from_bytes_with_discriminator(
        &commit_record_account.data,
    )
    .unwrap();
    assert_eq!(commit_record.account, args.delegated_account);
    assert_eq!(commit_record.identity, args.authority.pubkey());
    assert_eq!(commit_record.nonce, 1);

    let delegation_metadata_pda =
        delegation_metadata_pda_from_delegated_account(&args.delegated_account);
    let delegation_metadata_account = args
        .banks
        .get_account(delegation_metadata_pda)
        .await
        .unwrap()
        .unwrap();
    let delegation_metadata =
        DelegationMetadata::try_from_bytes_with_discriminator(
            &delegation_metadata_account.data,
        )
        .unwrap();
    assert_eq!(
        delegation_metadata.undelegation_requester,
        dlp_api::state::UndelegationRequester::Validator
    );
}

async fn commit_state_with_nonce(args: CommitStateWithNonceArgs<'_>) {
    let data = if args.delegated_account.eq(&DELEGATED_PDA_ID) {
        COMMIT_NEW_STATE_ACCOUNT_DATA.to_vec()
    } else {
        vec![]
    };
    let commit_args = CommitStateArgs {
        data,
        nonce: args.nonce,
        allow_undelegation: args.allow_undelegation,
        lamports: args.new_delegated_account_lamports,
    };

    let ix = dlp_api::instruction_builder::commit_state(
        args.authority.pubkey(),
        args.delegated_account,
        args.delegated_account_owner,
        commit_args,
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&args.fee_payer.pubkey()),
        &[&args.authority, &args.fee_payer],
        args.banks.get_latest_blockhash().await.unwrap(),
    );
    let res = args.banks.process_transaction(tx).await;

    assert!(res.is_ok(), "{} failed: {:?}", args.label, res);
}

struct FinalizeWithFeePayerArgs<'a> {
    banks: &'a mut BanksClient,
    authority: &'a Keypair,
    fee_payer: &'a Keypair,
    label: &'a str,
    delegated_account: Pubkey,
    owner_program: Pubkey,
}

async fn finalize_with_fee_payer(args: FinalizeWithFeePayerArgs<'_>) {
    let ix = dlp_api::instruction_builder::finalize(
        args.authority.pubkey(),
        args.delegated_account,
        args.owner_program,
        args.authority.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&args.fee_payer.pubkey()),
        &[&args.authority, &args.fee_payer],
        args.banks.get_latest_blockhash().await.unwrap(),
    );
    let res = args.banks.process_transaction(tx).await;

    assert!(res.is_ok(), "{} failed: {:?}", args.label, res);
}

#[derive(Debug)]
struct SetupProgramCommitTestEnvArgs {
    delegated_account_init_lamports: u64,
    delegated_account_current_lamports: u64,
    validator_vault_init_lamports: u64,
    delegated_account: Pubkey,
    owner_program: Pubkey,
}

async fn setup_program_for_commit_test_env(
    args: SetupProgramCommitTestEnvArgs,
) -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp_api::ID, None);
    program_test.prefer_bpf(true);

    let validator_keypair =
        crate::fixtures::keypair_from_bytes(&TEST_AUTHORITY);

    program_test.add_account(
        validator_keypair.pubkey(),
        Account {
            lamports: 10 * LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup a delegated PDA
    program_test.add_account(
        args.delegated_account,
        Account {
            lamports: args.delegated_account_current_lamports,
            data: vec![],
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated account metadata PDA
    let data = if args.owner_program.eq(&DELEGATED_PDA_OWNER_ID) {
        get_delegation_metadata_data(validator_keypair.pubkey(), None)
    } else {
        get_delegation_metadata_data_on_curve(validator_keypair.pubkey(), None)
    };
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&args.delegated_account),
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated record PDA
    let delegation_record_data = create_delegation_record_data(
        validator_keypair.pubkey(),
        args.owner_program,
        Some(args.delegated_account_init_lamports),
    );
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&args.delegated_account),
        Account {
            lamports: Rent::default()
                .minimum_balance(delegation_record_data.len()),
            data: delegation_record_data,
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the protocol fees vault
    program_test.add_account(
        fees_vault_pda(),
        Account {
            lamports: Rent::default().minimum_balance(0),
            data: vec![],
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the validator fees vault
    program_test.add_account(
        validator_fees_vault_pda_from_validator(&validator_keypair.pubkey()),
        Account {
            lamports: args.validator_vault_init_lamports,
            data: vec![],
            owner: dlp_api::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let data = read_file("tests/buffers/test_delegation.so");
    program_test.add_account(
        DELEGATED_PDA_OWNER_ID,
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: solana_sdk::bpf_loader::id(),
            executable: true,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, validator_keypair, blockhash)
}
