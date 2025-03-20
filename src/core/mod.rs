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
use std::fs;
use std::sync::Arc;
use std::sync::Mutex;

use std::net::SocketAddr;

use std::fs::File;

use serde::Deserialize;
use serde::Serialize;

use libstackerdb::StackerDBChunkData;

use clarity::vm::types::QualifiedContractIdentifier;

use crate::runner;
use crate::runner::stackerdb::StackerDBSession;

use crate::storage::StackerDBClient;
use crate::storage::Wrbpod;
use crate::storage::WrbpodAddress;

use crate::ui::render::Renderer;

use stacks_common::util::secp256k1::Secp256k1PrivateKey;

#[cfg(test)]
use crate::storage::tests::MockStackerDBClient;

use lazy_static::lazy_static;

pub mod config;
pub mod globals;

pub use crate::core::config::Config;
pub use crate::core::config::ConfigFile;
pub use crate::core::globals::Globals;
pub use crate::core::globals::GLOBALS;
pub use crate::core::globals::LOGFILE;

use crate::runner::bns::BNSResolver;
use crate::runner::bns::NodeBNSResolver;

use crate::runner::Runner;

/// Initialize global config
pub fn init(mainnet: bool, node_host: &str, node_port: u16) {
    match GLOBALS.lock() {
        Ok(mut globals) => {
            globals.reset();
            globals.set_config(Config::default(mainnet, node_host, node_port));
        }
        Err(_e) => {
            wrb_error!("FATAL: global mutex poisoned");
            panic!();
        }
    }
}

/// Initialize global config with config
pub fn init_config(conf: Config) {
    match GLOBALS.lock() {
        Ok(mut globals) => {
            globals.reset();
            globals.set_config(conf);
        }
        Err(_e) => {
            wrb_error!("FATAL: global mutex poisoned");
            panic!();
        }
    }
}

pub fn with_globals<F, R>(func: F) -> R
where
    F: FnOnce(&mut Globals) -> R,
{
    match GLOBALS.lock() {
        Ok(mut globals) => func(&mut (*globals)),
        Err(_e) => {
            wrb_error!("FATAL: global mutex poisoned");
            panic!();
        }
    }
}

pub fn with_global_config<F, R>(func: F) -> Option<R>
where
    F: FnOnce(&Config) -> R,
{
    with_globals(|globals| globals.config.as_ref().map(|cfg| func(cfg)))
}

/// Make a runner
pub fn make_runner() -> Runner {
    let (node_host, node_port) =
        with_global_config(|cfg| cfg.get_node_addr()).expect("FATAL: system not initialized");

    let (bns_contract_id, zonefile_contract_id, mock_stackerdb_paths) = with_global_config(|cfg| {
        (
            cfg.get_bns_contract_id(),
            cfg.get_zonefile_contract_id(),
            cfg.mock_stackerdb_paths().clone(),
        )
    })
    .expect("FATAL: system not initialized");

    let runner = Runner::new(bns_contract_id, zonefile_contract_id, node_host, node_port)
        .with_mock_stackerdb_paths(mock_stackerdb_paths);

    runner
}

/// Split a wrbsite name into its name and namespace
pub fn split_fqn(wrbsite_name: &str) -> Result<(String, String), String> {
    let mut wrbsite_split = wrbsite_name.split(".");
    let Some(name) = wrbsite_split.next() else {
        return Err("Malformed wrbsite name -- no '.'".to_string());
    };
    let Some(namespace) = wrbsite_split.next() else {
        return Err("Malformed wrbsite name -- no namespace".to_string());
    };
    Ok((name.to_string(), namespace.to_string()))
}

/// Resolve a name to its wrbsite and version.
/// Used in prod - uses NodeBNSResolver and StackerDBSession
pub fn wrbsite_load(wrbsite_name: &str) -> Result<(Vec<u8>, u32), String> {
    let (name, namespace) = split_fqn(wrbsite_name).map_err(|e_str| {
        format!(
            "Invalid fully qualified name; could not decode name and namespace: {}",
            &e_str
        )
    })?;

    let mut resolver = NodeBNSResolver::new();
    let mut runner = make_runner();

    let Some(home_node_addr) = runner
        .resolve_node()
        .map_err(|e| format!("Failed to resolve node: {:?}", &e))?
    else {
        return Err("Not connected to home node".into());
    };

    let (wrbsite_bytes, version) = runner
        .wrbsite_load_ext(
            &mut resolver,
            &name,
            &namespace,
            |contract_id: &QualifiedContractIdentifier, node_addr: &SocketAddr| {
                Runner::home_node_connect(contract_id, node_addr)
            },
            |contract_id: &QualifiedContractIdentifier, node_p2p_addr: &SocketAddr| {
                Runner::replica_node_connect(contract_id, &home_node_addr, node_p2p_addr)
            },
        )
        .map_err(|e| format!("Failed to load '{}': {:?}", wrbsite_name, &e))?
        .ok_or_else(|| format!("No wrbsite found for '{}'", wrbsite_name))?;

    Ok((wrbsite_bytes, version))
}

/// Load the wrbsite for the given name from the given source.
/// Returns the code bytes and version
/// Used in prod
pub fn load_wrbsite_source(
    wrbsite_name: &str,
    source: Option<String>,
) -> Result<(Vec<u8>, u32), String> {
    let Some(path) = source else {
        return wrbsite_load(wrbsite_name)
            .map_err(|e| format!("Failed to load '{}': {:?}", wrbsite_name, &e));
    };

    // treat source as a path to uncompressed clarity code
    let code = fs::read_to_string(&path).map_err(|e| format!("Invalid path: {}", &e))?;
    let bytes = Renderer::encode_bytes(code.as_bytes())
        .map_err(|e| format!("Failed to encode source code from '{}': {:?}", &path, &e))?;

    Ok((bytes, 0))
}
