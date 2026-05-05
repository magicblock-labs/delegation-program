use borsh_0_10::{BorshDeserialize, BorshSerialize};
use pinocchio::error::ProgramError;

use super::DelegateArgs;
use crate::{
    compact::{self, MAX_PUBKEYS},
    require, require_le,
};

#[derive(Debug, BorshSerialize, BorshDeserialize)]
#[borsh(crate = "borsh_0_10")]
pub struct DelegateWithActionsArgs {
    /// Standard delegation parameters.
    pub delegate: DelegateArgs,

    /// Compact post-delegation actions.
    pub actions: PostDelegationActions,
}

impl DelegateWithActionsArgs {
    pub fn parse(data: &[u8]) -> Result<DelegateWithActionsArgs, ProgramError> {
        let args = DelegateWithActionsArgs::try_from_slice(data)
            .map_err(|_| ProgramError::InvalidInstructionData)?;

        PostDelegationActions::validate(&args.actions)?;

        Ok(args)
    }
}

///
/// This struct is used both as instruction args and as persisted state: it is received as
/// instruction data and then stored in the delegation-record account immediately after
/// DelegationRecord.
///
/// In the basic form, PostDelegationActions is constructed from Vec<Instruction>. In that case,
/// both inserted_signers and inserted_non_signers are zero, and the pubkey indices used by
/// compact::AccountMeta are computed from this imagined pubkey list:
///
///   [signers.., non_signers..]
///
/// That is, the first non-signer pubkey index is signers.len().
///
/// In the advanced form, an existing PostDelegationActions value (usually provided by an off-chain
/// client) is combined with Vec<Instruction> (usually constructed on-chain) to produce a merged
/// PostDelegationActions via ClearTextWithInsertable::cleartext_with_insertable, which allows the
/// existing actions to be inserted in the provided Vec<_> at a specific index. In this case, both
/// inserted_signers and inserted_non_signers may be non-zero, and the imagined pubkey list takes
/// this form (old_signers and old_non_signers are from insertable PostDelegationActions):
///
///   [old_signers.., old_non_signers.., new_signers.., new_non_signers..]
///
/// That is, the first "new signer" comes after the old keys (old_signers + old_non_signers). Also,
/// the `signers` field is created from:
///
///  [old_signers.., new_signers..]
///
/// and `non-signers` field is created from:
///
///  [old_non_signers.., new_non_signers..]
///
/// Example,
///
/// ```ignore
///
/// let instructions: Vec<Instruction> = on_chain_instructions();
///
/// let merged_actions = instructions.cleartext_with_insertable(insertable_actions, 1);
///
/// ```
///
/// Assume insertable_actions has 2 signers and 3 non-signers pubkeys, then we have the
/// following index ordering and merged_actions.
///
/// | ----------------------- index ordering ----------------------------------
/// | inserted-signers | inserted-non-signers | new-signers | new-non-signers |
/// |  0 1             |  2 3 4               | 5           | 6 7             |
/// | ------------------------ merged actions ---------------------------------
/// | merged_actions.inserted_signers     = 2                                 |
/// | merged_actions.inserted_non_signers = 3                                 |
/// | merged_actions.signers              = [pk_0, pk_1, pk_5]                |
/// | merged_actions.non_signers          = [pk_2, pk_3, pk_4, pk_6, pk_7]    |
/// ---------------------------------------------------------------------------
///
#[derive(Debug, BorshSerialize, BorshDeserialize)]
#[borsh(crate = "borsh_0_10")]
pub struct PostDelegationActions {
    pub inserted_signers: u8,

    pub inserted_non_signers: u8,

    pub signers: Vec<[u8; 32]>,

    pub non_signers: Vec<MaybeEncryptedPubkey>,

    pub instructions: Vec<MaybeEncryptedInstruction>,
}

///
/// AddressIndex is a validated index, returned by validate_index()
///
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct AddressIndex(u8);

impl PostDelegationActions {
    pub fn validate(this: &PostDelegationActions) -> Result<(), ProgramError> {
        require_le!(
            this.inserted_signers as usize,
            this.signers.len(),
            ProgramError::InvalidInstructionData
        );

        require_le!(
            this.inserted_non_signers as usize,
            this.non_signers.len(),
            ProgramError::InvalidInstructionData
        );

        this.total_inserted()?;
        this.total_keys()?;

        Ok(())
    }

    pub fn total_inserted(&self) -> Result<u8, ProgramError> {
        self.inserted_signers
            .checked_add(self.inserted_non_signers)
            .and_then(|total| (total <= MAX_PUBKEYS).then_some(total))
            .ok_or(ProgramError::InvalidInstructionData)
    }

    pub fn total_keys(&self) -> Result<u8, ProgramError> {
        (self.signers.len() as u8)
            .checked_add(self.non_signers.len() as u8)
            .and_then(|total| (total <= MAX_PUBKEYS).then_some(total))
            .ok_or(ProgramError::InvalidInstructionData)
    }

    pub fn validate_index(
        &self,
        index: u8,
    ) -> Result<AddressIndex, ProgramError> {
        require!(
            index < self.total_keys()?,
            ProgramError::InvalidInstructionData
        );
        Ok(AddressIndex(index))
    }

    pub fn is_signer(&self, index: AddressIndex) -> Result<bool, ProgramError> {
        let index = index.0;
        Ok(index < self.inserted_signers
            || (index >= self.total_inserted()?
                && index
                    < self
                        .inserted_non_signers
                        .checked_add(self.signers.len() as u8)
                        .ok_or(ProgramError::InvalidInstructionData)?))
    }
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
#[borsh(crate = "borsh_0_10")]
pub struct MaybeEncryptedInstruction {
    pub program_id: u8,

    pub accounts: Vec<MaybeEncryptedAccountMeta>,

    pub data: MaybeEncryptedIxData,
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
#[cfg_attr(test, derive(PartialEq))]
#[borsh(crate = "borsh_0_10")]
pub enum MaybeEncryptedPubkey {
    ClearText([u8; 32]),
    Encrypted(EncryptedBuffer),
}

impl From<[u8; 32]> for MaybeEncryptedPubkey {
    fn from(pubkey: [u8; 32]) -> Self {
        Self::ClearText(pubkey)
    }
}

impl From<Vec<u8>> for MaybeEncryptedPubkey {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Encrypted(bytes.into())
    }
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
#[borsh(crate = "borsh_0_10")]
pub enum MaybeEncryptedAccountMeta {
    ClearText(compact::AccountMeta),
    Encrypted(EncryptedBuffer),
}

impl From<compact::AccountMeta> for MaybeEncryptedAccountMeta {
    fn from(account_meta: compact::AccountMeta) -> Self {
        Self::ClearText(account_meta)
    }
}

impl From<Vec<u8>> for MaybeEncryptedAccountMeta {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Encrypted(bytes.into())
    }
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
#[borsh(crate = "borsh_0_10")]
pub struct MaybeEncryptedIxData {
    pub prefix: Vec<u8>,
    pub suffix: EncryptedBuffer,
}

#[derive(Clone, Debug, Default, BorshSerialize, BorshDeserialize)]
#[cfg_attr(test, derive(PartialEq))]
#[borsh(crate = "borsh_0_10")]
pub struct EncryptedBuffer(Vec<u8>);

impl EncryptedBuffer {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl From<Vec<u8>> for EncryptedBuffer {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}
