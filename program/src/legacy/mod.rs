#[allow(unused_imports)]
pub(crate) use dpi::legacy::{
    account_size_class, args, consts, discriminator, pda, pod_view, state,
};
#[allow(unused_imports)]
pub(crate) use dpi::{
    commit_record_seeds_from_delegated_account,
    commit_state_seeds_from_delegated_account,
    delegate_buffer_seeds_from_delegated_account,
    delegation_metadata_seeds_from_delegated_account,
    delegation_record_seeds_from_delegated_account,
    ephemeral_balance_seeds_from_payer, fees_vault_seeds,
    magic_fee_vault_seeds_from_validator, program_config_seeds_from_program_id,
    undelegate_buffer_seeds_from_delegated_account,
    undelegation_request_seeds_from_delegated_account,
    validator_fees_vault_seeds_from_validator,
};
#[allow(unused_imports)]
pub(crate) use dpi::{
    require_initialized_pda, require_initialized_pda_fast, require_pda,
};

pub mod processor;
