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

use std::fs::File;

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

pub use crate::core::config::Config;
pub use crate::core::globals::Globals;
pub use crate::core::globals::GLOBALS;
pub use crate::core::globals::LOGFILE;

/// Initialize global config
pub fn init(mainnet: bool, node_host: &str, node_port: u16) {
    GLOBALS
        .lock()
        .unwrap()
        .set_config(Config::default(mainnet, node_host, node_port));
    
}

/// Initialize global config with config
pub fn init_config(conf: Config) {
    GLOBALS
        .lock()
        .unwrap()
        .set_config(conf)
}

pub fn with_globals<F, R>(func: F) -> R
where
    F: FnOnce(&mut Globals) -> R,
{
    match GLOBALS.lock() {
        Ok(mut globals) => {
            func(&mut (*globals))
        }
        Err(_e) => {
            wrb_error!("FATAL: global mutex poisoned");
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
