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
use std::fmt::Debug;
use std::fs;
use std::io::{stdin, stdout, Read};
use std::path::Path;
use std::process;
use std::str::FromStr;
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

pub use crate::core::load_wrbsite_source;
pub use crate::core::make_runner;
pub use crate::core::split_fqn;
pub use crate::core::wrbsite_load;

pub mod bns;
pub mod clar;
pub mod site;
pub mod wrbpod;

#[cfg(test)]
pub mod tests;

pub use crate::cli::bns::subcommand_bns;
pub use crate::cli::clar::subcommand_clarity;
pub use crate::cli::site::subcommand_site;
pub use crate::cli::wrbpod::subcommand_wrbpod;

pub fn consume_arg(
    args: &mut Vec<String>,
    argnames: &[&str],
    has_optarg: bool,
) -> Result<Option<String>, String> {
    if let Some(ref switch) = args
        .iter()
        .find(|ref arg| argnames.iter().find(|ref argname| argname == arg).is_some())
    {
        let idx = args
            .iter()
            .position(|ref arg| arg == switch)
            .expect("BUG: did not find the thing that was just found");
        let argval = if has_optarg {
            // following argument is the argument value
            if idx + 1 < args.len() {
                Some(args[idx + 1].clone())
            } else {
                // invalid usage -- expected argument
                return Err("Expected argument".to_string());
            }
        } else {
            // only care about presence of this option
            Some("".to_string())
        };

        args.remove(idx);
        if has_optarg {
            // also clear the argument
            args.remove(idx);
        }
        Ok(argval)
    } else {
        // not found
        Ok(None)
    }
}

pub fn usage(msg: &str) {
    let args: Vec<_> = env::args().collect();
    eprintln!("FATAL: {}", msg);
    eprintln!("Usage: {} [options] wrbsite", &args[0]);
    process::exit(1);
}

/// Open a StackerDB session to the home node
pub fn open_home_stackerdb_session(
    contract_id: QualifiedContractIdentifier,
) -> Result<Box<dyn StackerDBClient>, String> {
    let privkey = with_global_config(|cfg| cfg.private_key().clone())
        .ok_or("System is not initialized".to_string())?;
    let mut runner = make_runner();
    let home_stackerdb_client = runner
        .get_home_stackerdb_client(contract_id.clone(), privkey.clone())
        .map_err(|e| {
            format!(
                "Failed to instantiate StackerDB client to {}: {:?}",
                contract_id, &e
            )
        })?;

    Ok(home_stackerdb_client)
}

/// Open a StackerDB session to the replica node
pub fn open_replica_stackerdb_session(
    contract_id: QualifiedContractIdentifier,
) -> Result<Box<dyn StackerDBClient>, String> {
    let privkey = with_global_config(|cfg| cfg.private_key().clone())
        .ok_or("System is not initialized".to_string())?;
    let mut runner = make_runner();
    let replica_stackerdb_client = runner
        .get_replica_stackerdb_client(contract_id.clone(), privkey.clone())
        .map_err(|e| {
            format!(
                "Failed to instantiate StackerDB client to {}: {:?}",
                contract_id, &e
            )
        })?;

    Ok(replica_stackerdb_client)
}

/// Get code bytes from a contract as part of a CLI command
pub fn wrbsite_load_code_bytes(
    contract_id: &QualifiedContractIdentifier,
    slot_id: u32,
) -> Option<Vec<u8>> {
    let mut stackerdb_session =
        open_replica_stackerdb_session(contract_id.clone()).unwrap_or_else(|e| {
            eprintln!(
                "FATAL: failed to connect to StackerDB {} on replica node: {}",
                &contract_id, &e
            );
            process::exit(1);
        });

    let code_bytes_opt = stackerdb_session
        .get_latest_chunks(&[slot_id])
        .unwrap_or_else(|e| {
            eprintln!("FATAL: failed to get site code chunk: {:?}", &e);
            process::exit(1);
        })
        .pop()
        .expect("FATAL(BUG): no slot value returned");

    code_bytes_opt
}

/// Get the fee for a transaction and generate it
pub fn make_tx<F>(
    runner: &mut Runner,
    fee_opt: Option<u64>,
    mut tx_gen: F,
) -> Result<StacksTransaction, String>
where
    F: FnMut(u64) -> StacksTransaction,
{
    if let Some(fee) = fee_opt {
        return Ok(tx_gen(fee));
    }

    let tx_no_fee = tx_gen(0);
    let fee_estimate = runner.get_tx_fee(&tx_no_fee).map_err(|e| match e {
        RunnerError::NoFeeEstimate => {
            "Failed to learn fee estimate from node. Please pass a fee via -f or --fee.".to_string()
        }
        e => {
            format!("Failed to get transaction fee: {:?}", &e)
        }
    })?;

    if fee_estimate.estimations.len() == 0 {
        return Err("No fee estimation reported. Please pass a fee via -f or --fee.".into());
    }

    // take middle fee
    let est = fee_estimate.estimations.len() / 2;
    let tx_with_fee = tx_gen(fee_estimate.estimations[est].fee);
    Ok(tx_with_fee)
}

/// Determine which account(s) to poll to see if a tx confirmed
fn poll_tx_accounts(
    runner: &mut Runner,
    tx: &StacksTransaction,
) -> Result<(StacksAccount, Option<StacksAccount>), RunnerError> {
    let mainnet = tx.version == TransactionVersion::Mainnet;

    let origin_addr = if mainnet {
        tx.auth.origin().address_mainnet()
    } else {
        tx.auth.origin().address_testnet()
    }
    .to_account_principal();

    let origin_account = runner.get_account(&origin_addr)?;

    let sponsor_account = if let Some(sponsor) = tx.auth.sponsor() {
        let sponsor_addr = if mainnet {
            sponsor.address_mainnet()
        } else {
            sponsor.address_testnet()
        }
        .to_account_principal();
        Some(runner.get_account(&sponsor_addr)?)
    } else {
        None
    };

    Ok((origin_account, sponsor_account))
}

/// Post a transaction and wait for it to get confirmed
pub fn post_tx(runner: &mut Runner, tx: &StacksTransaction) -> Result<Txid, String> {
    let (origin_account_before, sponsor_account_before_opt) = poll_tx_accounts(runner, tx)
        .map_err(|e| format!("Failed to query transaction accounts: {:?}", &e))?;

    runner
        .post_tx(tx)
        .map_err(|e| format!("Failed to post transaction: {:?}", &e))?;

    eprint!("Sending tx {} and waiting for confirmation", &tx.txid());
    loop {
        thread::sleep(Duration::from_secs(1));

        let (origin_account, sponsor_account_opt) = poll_tx_accounts(runner, tx)
            .map_err(|e| format!("Failed to query transaction accounts: {:?}", &e))?;

        eprint!(".");
        if origin_account.nonce == origin_account_before.nonce {
            continue;
        }

        let Some(sponsor_account_before) = sponsor_account_before_opt.as_ref() else {
            break;
        };
        let Some(sponsor_account) = sponsor_account_opt.as_ref() else {
            break;
        };

        if sponsor_account_before.nonce == sponsor_account.nonce {
            continue;
        }

        break;
    }
    eprintln!();
    Ok(tx.txid())
}

/// get data from stdin or a file
pub fn load_from_file_or_stdin(path: &str) -> Vec<u8> {
    let data = if path == "-" {
        let mut fd = stdin();
        let mut bytes = vec![];
        fd.read_to_end(&mut bytes)
            .map_err(|e| {
                eprintln!("FATAL: failed to load from stdin: {:?}", &e);
                process::exit(1);
            })
            .unwrap();
        bytes
    } else {
        if let Err(e) = fs::metadata(path) {
            eprintln!("FATAL: could not open '{}': {:?}", path, &e);
            process::exit(1);
        }
        fs::read(path)
            .map_err(|e| {
                eprintln!("FATAL: failed to read from {}: {:?}", &path, &e);
                process::exit(1);
            })
            .unwrap()
    };
    data
}

/// Decode an integer
fn consume_int_arg<I>(argv: &mut Vec<String>, argnames: &[&str]) -> Option<I>
where
    I: FromStr<Err: Debug>,
{
    let value_opt = consume_arg(argv, argnames, true)
        .map_err(|e| {
            usage(&e);
            unreachable!()
        })
        .unwrap()
        .map(|u64_str| {
            u64_str
                .parse::<I>()
                .map_err(|e| {
                    usage(&format!("{:?}", &e));
                    unreachable!();
                })
                .unwrap()
        });

    value_opt
}

/// Decode u64
fn consume_u64(argv: &mut Vec<String>, argnames: &[&str]) -> Option<u64> {
    consume_int_arg::<u64>(argv, argnames)
}

/// Decode u32
fn consume_u32(argv: &mut Vec<String>, argnames: &[&str]) -> Option<u32> {
    consume_int_arg::<u32>(argv, argnames)
}

/// Decode a private key
/// TODO: expand to loading from a file or an environment variable
pub fn consume_private_key(
    argv: &mut Vec<String>,
    argnames: &[&str],
) -> Option<Secp256k1PrivateKey> {
    let privkey_opt = consume_arg(argv, &argnames, true)
        .map_err(|e| {
            usage(&e);
            unreachable!()
        })
        .unwrap()
        .map(|k_str| {
            Secp256k1PrivateKey::from_hex(&k_str)
                .map_err(|e| {
                    usage(&e);
                    unreachable!();
                })
                .unwrap()
        });

    privkey_opt
}
