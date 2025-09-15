use borsh::to_vec;
use solana_program::instruction::Instruction;
use solana_program::{
    bpf_loader_upgradeable, instruction::AccountMeta, pubkey::Pubkey, system_program,
};

use crate::args::SetFeesReceiverArgs;
use crate::discriminator::DlpDiscriminator;
use crate::pda::program_config_from_program_id;

/// Set the fees receiver.
/// See [crate::processor::process_set_fees_receiver] for docs.
pub fn set_fees_receiver(admin: Pubkey, fees_receiver: Pubkey, program: Pubkey) -> Instruction {
    let program_config_pda = program_config_from_program_id(&program);
    let delegation_program_data =
        Pubkey::find_program_address(&[crate::ID.as_ref()], &bpf_loader_upgradeable::id()).0;
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(admin, true),
            AccountMeta::new(program_config_pda, false),
            AccountMeta::new_readonly(program, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(delegation_program_data, false),
        ],
        data: [
            DlpDiscriminator::SetFeesReceiver.to_vec(),
            to_vec(&SetFeesReceiverArgs { fees_receiver }).unwrap(),
        ]
        .concat(),
    }
}
