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

use std::fs;

use rand::Rng;

use rusqlite::NO_PARAMS;
use rusqlite::Connection;

use crate::storage::util::*;

use clarity::vm::database::HeadersDB;

use stacks_common::types::chainstate::BlockHeaderHash;
use stacks_common::types::chainstate::BurnchainHeaderHash;
use stacks_common::types::chainstate::ConsensusHash;
use stacks_common::types::chainstate::StacksAddress;
use stacks_common::types::chainstate::StacksBlockId;
use stacks_common::types::chainstate::VRFSeed;
use stacks_common::util::hash::{Sha512Trunc256Sum, Hash160};

use stacks_common::util::get_epoch_time_secs;

use crate::storage::WrbHeadersDB;

/// Boilerplate implementation so we can interface the wrb DB with Clarity
impl HeadersDB for WrbHeadersDB {
    fn get_burn_header_hash_for_block(
        &self,
        id_bhh: &StacksBlockId,
    ) -> Option<BurnchainHeaderHash> {
        // mock it
        let conn = self.conn();
        if let Some(height) = get_wrb_block_height(&conn, id_bhh) {
            let mut bytes = [0u8; 32];
            bytes[0..8].copy_from_slice(&height.to_be_bytes());
            Some(BurnchainHeaderHash(bytes))
        } else {
            None
        }
    }

    fn get_consensus_hash_for_block(&self, id_bhh: &StacksBlockId) -> Option<ConsensusHash> {
        // mock it
        let conn = self.conn();
        if let Some(height) = get_wrb_block_height(&conn, id_bhh) {
            let mut bytes = [0u8; 20];
            bytes[0..8].copy_from_slice(&height.to_be_bytes());
            Some(ConsensusHash(bytes))
        } else {
            None
        }
    }

    fn get_vrf_seed_for_block(&self, id_bhh: &StacksBlockId) -> Option<VRFSeed> {
        let conn = self.conn();
        if let Some(height) = get_wrb_block_height(&conn, id_bhh) {
            let mut bytes = [0u8; 32];
            bytes[0..8].copy_from_slice(&height.to_be_bytes());
            Some(VRFSeed(bytes))
        } else {
            None
        }
    }

    fn get_stacks_block_header_hash_for_block(
        &self,
        id_bhh: &StacksBlockId,
    ) -> Option<BlockHeaderHash> {
        let conn = self.conn();
        if let Some(height) = get_wrb_block_height(&conn, id_bhh) {
            let mut bytes = [0u8; 32];
            bytes[0..8].copy_from_slice(&height.to_be_bytes());
            Some(BlockHeaderHash(bytes))
        } else {
            None
        }
    }

    fn get_burn_block_time_for_block(&self, id_bhh: &StacksBlockId) -> Option<u64> {
        let conn = self.conn();
        if let Some(height) = get_wrb_block_height(&conn, id_bhh) {
            Some(height)
        } else {
            None
        }
    }

    fn get_burn_block_height_for_block(&self, id_bhh: &StacksBlockId) -> Option<u32> {
        let conn = self.conn();
        if let Some(height) = get_wrb_block_height(&conn, id_bhh) {
            Some(height as u32)
        } else {
            None
        }
    }

    fn get_miner_address(&self, _id_bhh: &StacksBlockId) -> Option<StacksAddress> {
        None
    }

    fn get_burnchain_tokens_spent_for_block(&self, id_bhh: &StacksBlockId) -> Option<u128> {
        // if the block is defined at all, then return a constant
        get_wrb_block_height(&self.conn(), id_bhh).map(|_| 1)
    }

    fn get_burnchain_tokens_spent_for_winning_block(&self, id_bhh: &StacksBlockId) -> Option<u128> {
        // if the block is defined at all, then return a constant
        get_wrb_block_height(&self.conn(), id_bhh).map(|_| 1)
    }

    fn get_tokens_earned_for_block(&self, id_bhh: &StacksBlockId) -> Option<u128> {
        // if the block is defined at all, then return a constant
        get_wrb_block_height(&self.conn(), id_bhh).map(|_| 1)
    }
}


