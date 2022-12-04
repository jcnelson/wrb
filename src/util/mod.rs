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

pub mod sqlite;

use clarity::vm::costs::ExecutionCost;
use clarity::vm::ClarityVersion;

use stacks_common::types::StacksEpochId;
use stacks_common::types::chainstate::ConsensusHash;
use stacks_common::types::chainstate::BlockHeaderHash;

// copied from core
pub const FIRST_BURNCHAIN_CONSENSUS_HASH : ConsensusHash = ConsensusHash([0u8; 20]);
pub const FIRST_STACKS_BLOCK_HASH : BlockHeaderHash = BlockHeaderHash([0u8; 32]);

pub const CHAIN_ID_MAINNET: u32 = 0x77726201;
pub const CHAIN_ID_TESTNET: u32 = 0x80777262;

pub const BLOCK_LIMIT: ExecutionCost = ExecutionCost {
    write_length: 15_000_000,
    write_count: 15_000,
    read_length: 100_000_000,
    read_count: 15_000,
    runtime: 5_000_000_000,
};

pub const DEFAULT_WRB_EPOCH : StacksEpochId = StacksEpochId::Epoch21;
pub const DEFAULT_WRB_CLARITY_VERSION : ClarityVersion = ClarityVersion::Clarity2;
pub const DEFAULT_CHAIN_ID : u32 = CHAIN_ID_MAINNET;
