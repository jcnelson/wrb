// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2022 Stacks Open Internet Foundation
// Copyright (C) 2022 Jude Nelson
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::error;
use std::fmt;

use crate::vm::storage::WrbHeadersDB;
use clarity::vm::analysis::AnalysisDatabase;
use clarity::vm::database::BurnStateDB;
use clarity::vm::database::ClarityDatabase;
use clarity::vm::database::HeadersDB;

use stacks_common::types::chainstate::StacksBlockId;
use stacks_common::types::StacksEpochId;

use clarity::boot_util::boot_code_addr;
use clarity::vm::errors::Error as clarity_error;
use clarity::vm::types::QualifiedContractIdentifier;
use clarity::vm::ContractName;

use crate::vm::storage::Error as DBError;
use crate::vm::storage::WrbDB;

pub const STACKS_WRB_EPOCH: StacksEpochId = StacksEpochId::Epoch24;

pub mod clarity_vm;
pub mod contracts;
pub mod special;
pub mod storage;

pub use contracts::{BOOT_CODE, WRBLIB_CODE, WRB_CONTRACT, WRB_LOW_LEVEL_CONTRACT};

#[cfg(test)]
pub mod tests;

pub trait ClarityStorage {
    fn get_clarity_db<'a>(
        &'a mut self,
        headers_db: &'a dyn HeadersDB,
        burn_db: &'a dyn BurnStateDB,
    ) -> ClarityDatabase<'a>;
    fn get_analysis_db<'a>(&'a mut self) -> AnalysisDatabase<'a>;
}

pub struct ClarityVM {
    db: WrbDB,
    app_name: String,
    app_namespace: String,
    app_version: u32,
}

#[derive(Debug)]
pub enum Error {
    DB(DBError),
    Clarity(String),
    InvalidInput(String),
    NotInitialized,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::DB(ref e) => write!(f, "DB: {:?}", &e),
            Error::Clarity(ref e) => write!(f, "Clarity: {}", &e),
            Error::InvalidInput(ref e) => write!(f, "Invalid input: {}", &e),
            Error::NotInitialized => write!(f, "System not initialized"),
        }
    }
}

impl error::Error for Error {
    fn cause(&self) -> Option<&dyn error::Error> {
        match *self {
            Error::DB(ref e) => Some(e),
            Error::Clarity(ref _e) => None,
            Error::InvalidInput(ref _e) => None,
            Error::NotInitialized => None,
        }
    }
}

impl From<DBError> for Error {
    fn from(e: DBError) -> Self {
        Self::DB(e)
    }
}

impl From<clarity_error> for Error {
    fn from(e: clarity_error) -> Self {
        Self::Clarity(format!("{:?}", &e))
    }
}

pub const BOOT_BLOCK_ID: StacksBlockId = StacksBlockId([0xff; 32]);
pub const GENESIS_BLOCK_ID: StacksBlockId = StacksBlockId([0x00; 32]);
