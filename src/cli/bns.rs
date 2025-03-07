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
use crate::runner::bns::BNSNameOwner;
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

use clarity::vm::types::StandardPrincipalData;

use crate::cli::{
    consume_arg, consume_private_key, consume_u64, load_from_file_or_stdin, make_runner, make_tx,
    open_home_stackerdb_session, open_replica_stackerdb_session, post_tx, split_fqn, usage,
    wrbsite_load_code_bytes,
};

use serde;
use serde::Serialize;

#[derive(Serialize)]
struct PrettyPrintOwner {
    owner: String,
    renewal: u128,
}

impl From<BNSNameOwner> for PrettyPrintOwner {
    fn from(owner: BNSNameOwner) -> Self {
        Self {
            owner: owner.owner.to_string(),
            renewal: owner.renewal,
        }
    }
}

/// bns subcommand to resolve a BNS name to its zonefile.
/// Infallible; factored out here for use in multiple CLI commands.
pub fn subcommand_bns_resolve(wrbsite_name: &str) -> Option<Vec<u8>> {
    let (name, namespace) = split_fqn(wrbsite_name).unwrap_or_else(|e| {
        eprintln!("FATAL: could not decode name: {}", &e);
        process::exit(1);
    });

    let mut runner = make_runner();
    let mut bns_resolver = NodeBNSResolver::new();
    let zonefile_opt = bns_resolver
        .lookup(&mut runner, &name, &namespace)
        .map_err(|e| {
            eprintln!(
                "FATAL: failed to resolve zonefile of '{}': system error: {:?}",
                &wrbsite_name, &e
            );
            process::exit(1);
        })
        .unwrap()
        .map_err(|bns_e| {
            eprintln!(
                "FATAL: failed to resolve zonefile of '{}': BNS error: {:?}",
                &wrbsite_name, &bns_e
            );
            process::exit(1);
        })
        .unwrap()
        .zonefile;

    zonefile_opt
}

/// bns subcommand to query name owner
pub fn subcommand_bns_owner(wrbsite_name: &str) -> Option<BNSNameOwner> {
    let (name, namespace) = split_fqn(wrbsite_name).unwrap_or_else(|e| {
        eprintln!("FATAL: could not decode name: {}", &e);
        process::exit(1);
    });
    let mut runner = make_runner();
    let mut bns_resolver = NodeBNSResolver::new();

    let owner_opt = bns_resolver
        .get_owner(&mut runner, &name, &namespace)
        .map_err(|e| {
            eprintln!(
                "FATAL: failed to get owner of '{}': system error: {:?}",
                &wrbsite_name, &e
            );
            process::exit(1);
        })
        .unwrap();

    owner_opt
}

/// bns subcommand to query name price
fn subcommand_bns_price(wrbsite_name: &str) -> Option<u128> {
    let (name, namespace) = split_fqn(wrbsite_name).unwrap_or_else(|e| {
        eprintln!("FATAL: could not decode name: {}", &e);
        process::exit(1);
    });
    let mut runner = make_runner();
    let mut bns_resolver = NodeBNSResolver::new();

    let price_opt = bns_resolver
        .get_price(&mut runner, &name, &namespace)
        .map_err(|e| {
            eprintln!(
                "FATAL: failed to get price of '{}': system error: {:?}",
                &wrbsite_name, &e
            );
            process::exit(1);
        })
        .unwrap();

    price_opt
}

/// bns subcommand to fast-register a BNS name.
fn subcommand_bns_fast_register(
    wrbsite_name: &str,
    dry_run: bool,
    privkey_opt: Option<Secp256k1PrivateKey>,
    tx_fee_opt: Option<u64>,
) -> Option<Txid> {
    let (name, namespace) = split_fqn(wrbsite_name).unwrap_or_else(|e| {
        eprintln!("FATAL: could not decode name: {}", &e);
        process::exit(1);
    });
    let mut runner = make_runner();

    if let Some(owner) = subcommand_bns_owner(wrbsite_name) {
        eprintln!(
            "Name '{}' already registered. Owner is {}",
            wrbsite_name,
            serde_json::to_string(&owner).unwrap_or("(failed to serialize to JSON)".into())
        );
        return None;
    }

    let (privkey, mainnet) = with_global_config(|cfg| (cfg.private_key().clone(), cfg.mainnet()))
        .ok_or_else(|| panic!("System is not initialized"))
        .unwrap();

    let privkey = privkey_opt.unwrap_or(privkey);

    let stacks_addr = StacksAddress::p2pkh(mainnet, &StacksPublicKey::from_private(&privkey));
    let principal =
        StandardPrincipalData::new(stacks_addr.version(), stacks_addr.bytes().clone().0).unwrap();
    let account = runner
        .get_account(&principal.clone().into())
        .unwrap_or_else(|e| {
            panic!("FATAL: failed to look up account {}: {:?}", &principal, &e);
        });

    let bns_address = StacksAddress::new(
        runner.get_bns_contract_id().issuer.version(),
        Hash160(runner.get_bns_contract_id().issuer.1.clone()),
    )
    .expect("Infallible");

    // go fast-register
    let tx = make_tx(&mut runner, tx_fee_opt, |fee_rate| {
        make_contract_call(
            mainnet,
            &privkey,
            account.nonce,
            fee_rate,
            &bns_address,
            "BNS-V2",
            "name-claim-fast",
            &[
                Value::buff_from(name.as_bytes().to_vec())
                    .expect("FATAL: name could not be converted to a buffer"),
                Value::buff_from(namespace.as_bytes().to_vec())
                    .expect("FATAL: namespace could not be converted to a buffer"),
                Value::Principal(principal.clone().into()),
            ],
            // doing Allow here since the codebase only ever burns STX and mints the NFT; it never
            // jumps to undetermined code (like trait concretizations)
            TransactionPostConditionMode::Allow,
            vec![],
        )
        .expect("FATAL: could not make update-zonefile transaction")
    })
    .unwrap_or_else(|e| {
        eprintln!(
            "FATAL: failed to generate zonefile-update transaction: {}",
            &e
        );
        process::exit(1);
    });

    if dry_run {
        println!("{}", &to_hex(&tx.serialize_to_vec()));
        return Some(tx.txid());
    }

    let txid = post_tx(&mut runner, &tx).unwrap_or_else(|e| {
        wrb_debug!("{}", &to_hex(&tx.serialize_to_vec()));
        eprintln!("FATAL: failed to post zonefile-update transaction: {}", &e);
        process::exit(1);
    });

    Some(txid)
}

/// bns subcommand helper
/// Commands start at argv[2]
pub fn subcommand_bns(mut argv: Vec<String>) {
    if argv.len() < 3 {
        eprintln!("Usage: {} bns [subcommand] [options]", &argv[0]);
        process::exit(1);
    }
    let cmd = argv[2].clone();
    if cmd == "resolve" {
        if argv.len() < 4 {
            eprintln!("Usage: {} bns {} [-r|--raw-hex] NAME", &argv[0], &cmd);
            process::exit(1);
        }
        let raw = consume_arg(&mut argv, &["-r", "--raw-hex"], false)
            .map_err(|e| {
                usage(&e);
                unreachable!()
            })
            .unwrap();

        let wrbsite_name = argv[3].clone();
        let zonefile = subcommand_bns_resolve(&wrbsite_name)
            .or_else(|| {
                eprintln!("FATAL: BNS name '{}' has no zonefile", &wrbsite_name);
                process::exit(1);
            })
            .unwrap();

        if raw.is_some() {
            let zonefile_hex = to_hex(&zonefile);
            println!("{}", &zonefile_hex);
            return;
        }

        let zonefile_str = String::from_utf8_lossy(&zonefile);
        println!("{}", &zonefile_str);
        return;
    } else if cmd == "owner" {
        if argv.len() < 4 {
            eprintln!("Usage: {} bns {} NAME", &argv[0], &cmd);
            process::exit(1);
        }
        let wrbsite_name = argv[3].clone();
        let owner_opt = subcommand_bns_owner(&wrbsite_name);

        if let Some(owner) = owner_opt {
            println!(
                "{}",
                serde_json::to_string(&PrettyPrintOwner::from(owner)).unwrap_or_else(|_| {
                    eprintln!("(name is owned, but failed to serialize to JSON)");
                    process::exit(1);
                })
            );
        } else {
            println!("Name is available");
        }

        return;
    } else if cmd == "price" {
        if argv.len() < 4 {
            eprintln!("Usage: {} bns {} NAME", &argv[0], &cmd);
            process::exit(1);
        }
        let wrbsite_name = argv[3].clone();
        let price_opt = subcommand_bns_price(&wrbsite_name);

        if let Some(price) = price_opt {
            println!("{}", price);
        } else {
            eprintln!("FATAL: unable to query name price. Namespace does not exist");
            process::exit(1);
        }
        return;
    } else if cmd == "fast-register" {
        if argv.len() < 4 {
            eprintln!(
                "Usage: {} bns {} [-k|--private-key KEY] [-f|--fee FEE] [-n|--dry-run] NAME",
                &argv[0], &cmd
            );
            process::exit(1);
        }
        let dry_run = consume_arg(&mut argv, &["-n", "--dry-run"], false)
            .map_err(|e| {
                usage(&e);
                unreachable!()
            })
            .unwrap();

        let privkey_opt = consume_private_key(&mut argv, &["-k", "--private-key"]);
        let tx_fee_opt = consume_u64(&mut argv, &["-f", "--fee"]);
        let wrbsite_name = argv[3].clone();

        let txid_opt =
            subcommand_bns_fast_register(&wrbsite_name, dry_run.is_some(), privkey_opt, tx_fee_opt);
        let Some(txid) = txid_opt else {
            process::exit(1);
        };
        println!("{}", &txid);
        return;
    }

    eprintln!("Unrecognized `bns` command '{}'", &cmd);
    process::exit(1);
}
