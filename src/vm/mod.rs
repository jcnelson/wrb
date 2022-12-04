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

use crate::storage::WrbHeadersDB;
use clarity::vm::database::HeadersDB;
use clarity::vm::database::BurnStateDB;
use clarity::vm::database::ClarityDatabase;
use clarity::vm::analysis::AnalysisDatabase;

use stacks_common::types::chainstate::StacksBlockId;

use crate::storage::WrbDB;

pub mod clarity_vm;

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
    db: WrbDB
}

pub const BOOT_CODE : &'static [(&'static str, &'static str)] = &[
    (
        "wrb",
        r#"
        (begin
            (print "Wrb is not the Web")
        )"#
    )
];

pub const BOOT_BLOCK_ID : StacksBlockId = StacksBlockId([0xff; 32]);
pub const GENESIS_BLOCK_ID : StacksBlockId = StacksBlockId([0x00; 32]);
