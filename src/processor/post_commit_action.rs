use crate::ephemeral_balance_seeds_from_payer;
use crate::processor::utils::loaders::{load_owned_pda, load_pda, load_signer};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::account_info::next_account_info;
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::instruction::Instruction;
use solana_program::pubkey::Pubkey;
use std::io::{Read, Write};
use std::marker::PhantomData;
// commit_and_undelegate_with_ix(payer, accountss, magic_porgam, )

#[derive(BorshSerialize, BorshDeserialize)]
struct PostCommitArgs {
    // TODO: this shall be passed, since excrow there could be multiple escrows
    // TODO: do we even need escrow for post commit action?
    escrow_index: u8,

    // TODO: could be arbitrary data
    transaction_message: Vec<u8>,
    // TODO: do we need this?
    destination_program: Pubkey,
}

/// Unvalidated instruction data, must be treated as untrusted.
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct TransactionMessage {
    /// The number of signer pubkeys in the account_keys vec.
    pub num_signers: u8,
    /// The number of writable signer pubkeys in the account_keys vec.
    pub num_writable_signers: u8,
    /// The number of writable non-signer pubkeys in the account_keys vec.
    pub num_writable_non_signers: u8,
    /// The list of unique account public keys (including program IDs) that will be used in the provided instructions.
    pub account_keys: SmallVec<u8, Pubkey>,
    /// The list of instructions to execute.
    pub instructions: SmallVec<u8, CompiledInstruction>,
    /// List of address table lookups used to load additional accounts
    /// for this transaction.
    pub address_table_lookups: SmallVec<u8, MessageAddressTableLookup>,
}

fn process_post_commit_action(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let args = PostCommitArgs::try_from_slice(data)?;
    let transaction_message =
        TransactionMessage::try_from_slice(args.transaction_message.as_slice())?;

    let accounts_iter = &mut accounts.iter();
    let validator = next_account_info(accounts_iter)?;
    let delegated_account = next_account_info(accounts_iter)?;
    let escrow_account = next_account_info(accounts_iter)?;

    load_signer(validator, "validator")?;
    // This is action on commited account, hence shall be still owned by delegation program
    load_owned_pda(delegated_account, &crate::ID, "delegated account")?;
    // Escrow account always owned by delegation program
    // TODO: questionable if it has to actually exist
    // in some cases it just has to testify to the provenance.
    // hence user approve it,
    // load_owned_pda(escrow_account, &crate::ID, "escrow account")?;

    // verify passed escrow_account derived from delegated_account
    let bump_ephemeral_balance = load_pda(
        escrow_account,
        ephemeral_balance_seeds_from_payer!(delegated_account.key, args.escrow_index),
        &crate::id(),
        true,
        "ephemeral balance",
    )?;

    Ok(())
}

// Concise serialization schema for instructions that make up transaction.
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct CompiledInstruction {
    pub program_id_index: u8,
    /// Indices into the tx's `account_keys` list indicating which accounts to pass to the instruction.
    pub account_indexes: SmallVec<u8, u8>,
    /// Instruction data.
    pub data: SmallVec<u16, u8>,
}

/// Address table lookups describe an on-chain address lookup table to use
/// for loading more readonly and writable accounts in a single tx.
#[derive(BorshSerialize, BorshDeserialize, Clone)]
pub struct MessageAddressTableLookup {
    /// Address lookup table account key
    pub account_key: Pubkey,
    /// List of indexes used to load writable account addresses
    pub writable_indexes: SmallVec<u8, u8>,
    /// List of indexes used to load readonly account addresses
    pub readonly_indexes: SmallVec<u8, u8>,
}

/// Concise serialization schema for vectors where the length can be represented
/// by any type `L` (typically unsigned integer like `u8` or `u16`)
/// that implements BorshDeserialize and can be converted to `u32`.
#[derive(Clone, Debug, Default)]
pub struct SmallVec<L, T>(Vec<T>, PhantomData<L>);

impl<L, T> SmallVec<L, T> {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<L, T> From<SmallVec<L, T>> for Vec<T> {
    fn from(val: SmallVec<L, T>) -> Self {
        val.0
    }
}

impl<L, T> From<Vec<T>> for SmallVec<L, T> {
    fn from(val: Vec<T>) -> Self {
        Self(val, PhantomData)
    }
}

impl<T: BorshSerialize> BorshSerialize for SmallVec<u8, T> {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let len = u8::try_from(self.len()).map_err(|_| std::io::ErrorKind::InvalidInput)?;
        // Write the length of the vector as u8.
        writer.write_all(&len.to_le_bytes())?;

        // Write the vector elements.
        serialize_slice(&self.0, writer)
    }
}

impl<T: BorshSerialize> BorshSerialize for SmallVec<u16, T> {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let len = u16::try_from(self.len()).map_err(|_| std::io::ErrorKind::InvalidInput)?;
        // Write the length of the vector as u16.
        writer.write_all(&len.to_le_bytes())?;

        // Write the vector elements.
        serialize_slice(&self.0, writer)
    }
}

impl<L, T> BorshDeserialize for SmallVec<L, T>
where
    L: BorshDeserialize + Into<u32>,
    T: BorshDeserialize,
{
    /// This implementation almost exactly matches standard implementation of
    /// `Vec<T>::deserialize` except that it uses `L` instead of `u32` for the length,
    /// and doesn't include `unsafe` code.
    fn deserialize_reader<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let len: u32 = L::deserialize_reader(reader)?.into();

        let vec = if len == 0 {
            Vec::new()
        } else if let Some(vec_bytes) = T::vec_from_reader(len, reader)? {
            vec_bytes
        } else {
            let mut result = Vec::with_capacity(hint::cautious::<T>(len));
            for _ in 0..len {
                result.push(T::deserialize_reader(reader)?);
            }
            result
        };

        Ok(SmallVec(vec, PhantomData))
    }
}

// This is copy-pasted from borsh::de::hint;
mod hint {
    #[inline]
    pub fn cautious<T>(hint: u32) -> usize {
        let el_size = core::mem::size_of::<T>() as u32;
        core::cmp::max(core::cmp::min(hint, 4096 / el_size), 1) as usize
    }
}

/// Helper method that is used to serialize a slice of data (without the length marker).
/// Copied from borsh::ser::serialize_slice.
#[inline]
fn serialize_slice<T: BorshSerialize, W: Write>(data: &[T], writer: &mut W) -> std::io::Result<()> {
    if let Some(u8_slice) = T::u8_slice(data) {
        writer.write_all(u8_slice)?;
    } else {
        for item in data {
            item.serialize(writer)?;
        }
    }
    Ok(())
}
