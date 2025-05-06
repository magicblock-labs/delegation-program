use crate::discriminator::DlpDiscriminator;
use crate::instruction_builder::undelegate;
use solana_program::instruction::Instruction;
use solana_program::pubkey::Pubkey;

/// Builds an undelegate instruction.
/// See [crate::processor::process_undelegate] for docs.
#[allow(clippy::too_many_arguments)]
pub fn undelegate_ephemeral_balance(
    validator: Pubkey,
    delegated_account: Pubkey,
    rent_reimbursement: Pubkey,
) -> Instruction {
    let mut ix = undelegate(validator, delegated_account, crate::ID, rent_reimbursement);
    ix.data = DlpDiscriminator::UndelegateEphemeralBalance.to_vec();

    ix
}
