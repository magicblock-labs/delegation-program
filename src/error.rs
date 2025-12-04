use num_enum::IntoPrimitive;
use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, IntoPrimitive)]
#[repr(u32)]
pub enum DlpError {
    #[error("Invalid Authority")]
    InvalidAuthority = 0,
    #[error("Account cannot be undelegated, is_undelegatable is false")]
    NotUndelegatable = 1,
    #[error("Unauthorized Operation")]
    Unauthorized = 2,
    #[error("Invalid Authority for the current target program")]
    InvalidAuthorityForProgram = 3,
    #[error("Delegated account does not match the expected account")]
    InvalidDelegatedAccount = 4,
    #[error("Delegated account is not in a valid state")]
    InvalidDelegatedState = 5,
    #[error("Reimbursement account does not match the expected account")]
    InvalidReimbursementAccount = 6,
    #[error("Invalid account data after CPI")]
    InvalidAccountDataAfterCPI = 7,
    #[error("Invalid validator balance after CPI")]
    InvalidValidatorBalanceAfterCPI = 8,
    #[error("Invalid reimbursement address for delegation rent")]
    InvalidReimbursementAddressForDelegationRent = 9,
    #[error("Authority is invalid for the delegated account program owner")]
    InvalidWhitelistProgramConfig = 10,
    #[error("Account already undelegated")]
    AlreadyUndelegated = 11,
    #[error("Commit is out of order")]
    NonceOutOfOrder = 12,
    #[error("Computation overflow detected")]
    Overflow = 13,
    #[error("Too many seeds")]
    TooManySeeds = 14,
    #[error("Invalid length of diff passed to DiffSet::try_new")]
    InvalidDiff = 15,
    #[error("Diff is not properly aligned")]
    InvalidDiffAlignment = 16,
    #[error("MergeDiff precondition did not meet")]
    MergeDiffError = 17,
    #[error("Commit state PDA invalid seeds")]
    CommitStateInvalidSeeds = 18,
    #[error("Commit state PDA invalid account owner")]
    CommitStateInvalidAccountOwner = 19,
    #[error("Commit state PDA is already initialized")]
    CommitStateAlreadyInitialized = 20,
    #[error("Commit state PDA immutable")]
    CommitStateImmutable = 21,
    #[error("Commit record PDA invalid seeds")]
    CommitRecordInvalidSeeds = 22,
    #[error("Commit record PDA invalid account owner")]
    CommitRecordInvalidAccountOwner = 23,
    #[error("Commit record PDA is already initialized")]
    CommitRecordAlreadyInitialized = 24,
    #[error("Commit record PDA immutable")]
    CommitRecordImmutable = 25,
    #[error("Delegation record PDA invalid seeds")]
    DelegationRecordInvalidSeeds = 26,
    #[error("Delegation record PDA invalid account owner")]
    DelegationRecordInvalidAccountOwner = 27,
    #[error("Delegation record PDA is already initialized")]
    DelegationRecordAlreadyInitialized = 28,
    #[error("Delegation record PDA immutable")]
    DelegationRecordImmutable = 29,
    #[error("Delegation metadata PDA invalid seeds")]
    DelegationMetadataInvalidSeeds = 30,
    #[error("Delegation metadata PDA invalid account owner")]
    DelegationMetadataInvalidAccountOwner = 31,
    #[error("Delegation metadata PDA is already initialized")]
    DelegationMetadataAlreadyInitialized = 32,
    #[error("Delegation metadata PDA immutable")]
    DelegationMetadataImmutable = 33,
    #[error("Undelegate buffer PDA invalid seeds")]
    UndelegateBufferInvalidSeeds = 34,
    #[error("Undelegate buffer PDA invalid account owner")]
    UndelegateBufferInvalidAccountOwner = 35,
    #[error("Undelegate buffer PDA is already initialized")]
    UndelegateBufferAlreadyInitialized = 36,
    #[error("Undelegate buffer PDA immutable")]
    UndelegateBufferImmutable = 37,
}

impl From<DlpError> for ProgramError {
    fn from(e: DlpError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl From<DlpError> for pinocchio::program_error::ProgramError {
    fn from(e: DlpError) -> Self {
        pinocchio::program_error::ProgramError::Custom(e as u32)
    }
}
