// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2025 Stacks Open Internet Foundation
// Copyright (C) 2025 Jude Nelson
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

use std::env;
use std::fs;
use std::io::{stdin, stdout, Read};
use std::path::Path;
use std::process;
use std::thread;
use std::time::Duration;

use crate::core::Config;
use crate::runner::bns::BNSResolver;
use crate::runner::bns::NodeBNSResolver;
use crate::runner::site::WrbTxtRecord;
use crate::runner::site::WrbTxtRecordV1;
use crate::runner::site::ZonefileResourceRecord;
use crate::runner::Error as RunnerError;
use crate::runner::Runner;

use crate::runner::tx::StacksAccount;
use crate::tx::TransactionVersion;
use crate::tx::Txid;

use crate::ui::events::WrbChannels;
use crate::ui::events::WrbEvent;
use crate::ui::Renderer;
use crate::viewer::Viewer;
use crate::vm::ClarityVM;

use crate::util::privkey_to_principal;
use crate::util::{DEFAULT_WRB_CLARITY_VERSION, DEFAULT_WRB_EPOCH};

use crate::storage::StackerDBClient;
use crate::storage::Wrbpod;
use crate::storage::WrbpodSlices;
use crate::storage::WrbpodSuperblock;

use crate::core::globals::redirect_logfile;
use crate::core::with_global_config;
use crate::core::with_globals;

use crate::runner::stackerdb::StackerDBSession;

use crate::vm::clarity_vm::vm_execute;

use crate::tx::{
    make_contract_call, StacksTransaction, TransactionPostCondition, TransactionPostConditionMode,
};

use clarity::vm::types::QualifiedContractIdentifier;
use clarity::vm::types::StacksAddressExtensions;
use clarity::vm::types::TupleData;
use clarity::vm::ClarityName;
use clarity::vm::Value;

use stacks_common::address::{
    C32_ADDRESS_VERSION_MAINNET_SINGLESIG, C32_ADDRESS_VERSION_TESTNET_SINGLESIG,
};
use stacks_common::types::chainstate::StacksAddress;
use stacks_common::types::chainstate::StacksPublicKey;
use stacks_common::util::hash::hex_bytes;
use stacks_common::util::hash::to_hex;
use stacks_common::util::hash::Hash160;
use stacks_common::util::secp256k1::Secp256k1PrivateKey;

use crate::stacks_common::codec::StacksMessageCodec;

use libstackerdb::StackerDBChunkAckData;
use libstackerdb::StackerDBChunkData;

use crate::cli::{
    consume_arg, load_from_file_or_stdin, make_runner, make_tx, open_home_stackerdb_session,
    open_replica_stackerdb_session, post_tx, split_fqn, usage, wrbsite_load_code_bytes,
};

fn inner_json_to_clarity(json_value: serde_json::Value) -> Result<Value, String> {
    match json_value {
        serde_json::Value::Null => Ok(Value::none()),
        serde_json::Value::Bool(val) => Ok(Value::Bool(val)),
        serde_json::Value::Number(num) => {
            if num.is_i64() {
                Ok(Value::Int(i128::from(
                    num.as_i64().ok_or(format!("Not an i64: {:?}", &num))?,
                )))
            } else if num.is_u64() {
                Ok(Value::UInt(u128::from(
                    num.as_u64().ok_or(format!("Not an i64: {:?}", &num))?,
                )))
            } else {
                Err(format!("Could not decode as u64 or i64: {:?}", &num))
            }
        }
        serde_json::Value::String(s) => {
            let value = vm_execute(&s, DEFAULT_WRB_CLARITY_VERSION)
                .map_err(|e| format!("Could not execute string '{}': {:?}", &s, &e))?
                .ok_or_else(|| format!("Failed to evaluate string '{}' to a value", &s))?;
            Ok(value)
        }
        serde_json::Value::Array(value_vec) => {
            let mut clarity_values = vec![];
            for value in value_vec {
                let clarity_val = inner_json_to_clarity(value)?;
                clarity_values.push(clarity_val);
            }
            Ok(Value::cons_list(clarity_values, &DEFAULT_WRB_EPOCH)
                .map_err(|e| format!("Could not build Clarity list: {:?}", &e))?)
        }
        serde_json::Value::Object(objs_map) => {
            let mut clarity_values = vec![];
            for (obj_name, obj_value) in objs_map.into_iter() {
                let clarity_name = ClarityName::try_from(obj_name.as_str())
                    .map_err(|e| format!("Could not build Clarity tuple name: {:?}", &e))?;
                let clarity_val = inner_json_to_clarity(obj_value)?;
                clarity_values.push((clarity_name, clarity_val));
            }
            let tuple_data = TupleData::from_data(clarity_values)
                .map_err(|e| format!("Could not build tuple from values: {:?}", &e))?;
            Ok(Value::Tuple(tuple_data))
        }
    }
}

/// Convert JSON into a Clarity tuple
pub fn json_to_clarity<R: Read>(fd: &mut R) -> Result<Value, String> {
    let json_obj: serde_json::Value =
        serde_json::from_reader(fd).map_err(|e| format!("Failed to decode JSON: {:?}", &e))?;

    inner_json_to_clarity(json_obj)
}

/// clarity subcommand handler.
/// Commands start at argv[2]
pub fn subcommand_clarity(argv: Vec<String>) {
    if argv.len() < 3 {
        eprintln!("Usage: {} clarity [subcommand] [options]", &argv[0]);
        process::exit(1);
    }

    let cmd = argv[2].clone();
    if cmd == "encode-json" {
        if argv.len() < 4 {
            eprintln!("Usage: {} clarity encode-json JSON", &argv[0]);
            process::exit(1);
        }

        let json_str = argv[3].clone();
        let json_res = if json_str == "-" {
            json_to_clarity(&mut stdin())
        } else {
            json_to_clarity(&mut json_str.as_bytes())
        };

        let clarity_val = json_res
            .map_err(|e| {
                eprintln!("Failed to encode from JSON: {:?}", &e);
                process::exit(1)
            })
            .unwrap();

        println!(
            "{}",
            clarity_val
                .serialize_to_hex()
                .map_err(|e| {
                    eprintln!("Failed to serialize Clarity value to hex: {:?}", &e);
                    process::exit(1)
                })
                .unwrap()
        );

        return;
    }

    eprintln!("Unrecognized `clarity` command '{}'", &cmd);
    process::exit(1);
}
