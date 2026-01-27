const KB: u32 = 1024;
const MB: u32 = 1024 * KB;

//
// PERFORMANCE TIP: if a single transaction contains N dlp-instructions, add
// DLP_PROGRAM_DATA_SIZE_CLASS once, not N times. So consider doing something like this:
//
//  let size_budget = commit_diff_size_budget(blah)
//      + finalize_size_budget(blah)
//      + undelegate_size_budget(blah)
//      - 2 * DLP_PROGRAM_DATA_SIZE_CLASS.size_budget();
//
//  That is because each *_size_budget() function includes this constant, so callers must subtract (N-1) instances
//  when combining multiple instructions.
//
pub const DLP_PROGRAM_DATA_SIZE_CLASS: AccountSizeClass =
    AccountSizeClass::Dynamic(350 * KB);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AccountSizeClass {
    // account size <= 256 B
    //
    // - sysvars (rent, clock, epoch schedule)
    // - SPL token account and ATA
    // - SPL token mint
    // - delegation_metadata_pda
    // - delegation_record_pda
    // - commit_record_pda
    // - program_config_pda
    // - validator
    // - fees_vault_pda
    Tiny,

    // account size <= 1 KB
    //
    // - sysvars (recent blockhash)
    Small,

    // account size <= 8 KB
    //
    // - sysvars (instructions)
    Medium,

    // account size <= 64 KB
    //
    // - sysvars (state history)
    Large,

    // account size <= 256 KB
    //
    ExtraLarge,

    // account size <= 1 MB
    Huge,

    // any legal value
    Dynamic(u32),
}

impl AccountSizeClass {
    pub const fn size_budget(self) -> u32 {
        match self {
            Self::Tiny => 256,
            Self::Small => KB,
            Self::Medium => 8 * KB,
            Self::Large => 64 * KB,
            Self::ExtraLarge => 256 * KB,
            Self::Huge => MB,
            Self::Dynamic(n) => n,
        }
    }
}

pub fn total_size_budget(classes: &[AccountSizeClass]) -> u32 {
    classes.iter().map(|f| f.size_budget()).sum()
}
