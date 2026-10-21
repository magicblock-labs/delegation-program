use pinocchio::{AccountView, ProgramResult};

use crate::error::DlpError;

pub(crate) mod pda;

pub trait LamportsOperation {
    fn lamports_increment_by(&self, value: u64) -> ProgramResult;
    fn lamports_decrement_by(&self, value: u64) -> ProgramResult;
}

impl LamportsOperation for AccountView {
    fn lamports_increment_by(&self, value: u64) -> ProgramResult {
        self.set_lamports(
            self.lamports()
                .checked_add(value)
                .ok_or(DlpError::Overflow)?,
        );
        Ok(())
    }
    fn lamports_decrement_by(&self, value: u64) -> ProgramResult {
        self.set_lamports(
            self.lamports()
                .checked_sub(value)
                .ok_or(DlpError::Overflow)?,
        );
        Ok(())
    }
}
