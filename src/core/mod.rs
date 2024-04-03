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

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use serde::Deserialize;
use serde::Serialize;

use libstackerdb::StackerDBChunkData;

use crate::storage::StackerDBClient;
use crate::runner::stackerdb::StackerDBSession;
use crate::runner::bns::BNSNameRecord;
use crate::storage::Wrbpod;

use stacks_common::util::secp256k1::Secp256k1PrivateKey;

#[cfg(test)]
use crate::storage::tests::MockStackerDBClient;

use lazy_static::lazy_static;

pub mod config;
pub mod globals;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// Where the helper programs live
    helper_programs_dir: String,
    /// Where the node helper lives
    wrb_wallet_helper: String,
    /// where the sigin helper lives
    wrb_signin_helper: String,
    /// mainnet or testnet
    mainnet: bool,
    /// maximum attachment size
    max_attachment_size: usize,
    /// number of columns
    num_columns: usize,
    /// node host
    node_host: String,
    /// node port
    node_port: u16,
    /// identity key
    private_key: Secp256k1PrivateKey
}

/// Globally-accessible state that is hard to pass around otherwise
pub struct Globals {
    pub config: Option<Config>,
    /// Maps session IDs to wrbpod state
    pub wrbpod_sessions: HashMap<u128, Wrbpod>,
}

lazy_static! {
    pub static ref GLOBALS: Mutex<Globals> = Mutex::new(Globals {
        config: None,
        wrbpod_sessions: HashMap::new()
    });
}

/// Initialize global config
pub fn init(mainnet: bool, node_host: &str, node_port: u16) {
    GLOBALS
        .lock()
        .unwrap()
        .set_config(Config::default(mainnet, node_host, node_port));
}

pub fn with_globals<F, R>(func: F) -> R
where
    F: FnOnce(&mut Globals) -> R,
{
    match GLOBALS.lock() {
        Ok(mut globals) => func(&mut (*globals)),
        Err(_e) => {
            error!("FATAL: global mutex poisoned");
            panic!();
        }
    }
}

pub fn with_global_config<F, R>(func: F) -> Option<R>
where
    F: FnOnce(&Config) -> R
{
    with_globals(|globals| globals.config.as_ref().map(|cfg| func(cfg)))
}
