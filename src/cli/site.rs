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
    consume_arg, consume_private_key, consume_u64, load_from_file_or_stdin, make_runner, make_tx,
    open_home_stackerdb_session, open_replica_stackerdb_session, post_tx, split_fqn, usage,
    wrbsite_load_code_bytes,
};

use crate::cli::bns::subcommand_bns_owner;
use crate::cli::bns::subcommand_bns_resolve;

/// site upload
fn subcommand_site_upload(
    contract_id: &QualifiedContractIdentifier,
    slot_id: u32,
    path_to_code: String,
) -> StackerDBChunkAckData {
    let code = load_from_file_or_stdin(&path_to_code);

    let code_bytes = Renderer::encode_bytes(&code).unwrap_or_else(|e| {
        eprintln!("FATAL: failed to encode site code: {:?}", &e);
        process::exit(1);
    });

    let mut stackerdb_session =
        open_replica_stackerdb_session(contract_id.clone()).unwrap_or_else(|e| {
            eprintln!(
                "FATAL: failed to connect to StackerDB {} on replica node: {}",
                contract_id, &e
            );
            process::exit(1);
        });

    let slots_metadata = stackerdb_session.list_chunks().unwrap_or_else(|e| {
        eprintln!(
            "FATAL: failed to list chunks on StackerDB {} on replica node {}: {}",
            contract_id,
            &stackerdb_session.get_host(),
            &e
        );
        process::exit(1);
    });

    let slot_version = slots_metadata
        .get(slot_id as usize)
        .map(|slot_md| slot_md.slot_version)
        .unwrap_or_else(|| {
            eprintln!(
                "FATAL: no such StackerDB slot {} in {}",
                slot_id, contract_id
            );
            process::exit(1);
        });

    let privkey =
        with_global_config(|cfg| cfg.private_key().clone()).expect("System is not initialized");

    let mut chunk_data = StackerDBChunkData::new(slot_id, slot_version + 1, code_bytes);
    chunk_data.sign(&privkey).unwrap_or_else(|e| {
        eprintln!("FATAL: failed to sign chunk: {:?}", &e);
        process::exit(1);
    });

    let ack = stackerdb_session.put_chunk(chunk_data).unwrap_or_else(|e| {
        eprintln!("FATAL: failed to upload site code chunk: {:?}", &e);
        process::exit(1);
    });

    ack
}

/// site subcommand download
fn subcommand_site_download(contract_id: &QualifiedContractIdentifier, slot_id: u32) -> Vec<u8> {
    let code_bytes = wrbsite_load_code_bytes(contract_id, slot_id).unwrap_or_else(|| {
        eprintln!(
            "FATAL: no code for slot {} in StackerDB {}",
            slot_id, contract_id
        );
        process::exit(1);
    });

    let code = Renderer::decode_bytes(&code_bytes).unwrap_or_else(|e| {
        eprintln!("FATAL: failed to decode site code: {:?}", &e);
        process::exit(1);
    });

    code
}

/// site subcommand publish
fn subcommand_site_publish(
    contract_id: &QualifiedContractIdentifier,
    slot_id: u32,
    wrbsite_name: String,
    dry_run: bool,
    raw: bool,
    name_privkey_opt: Option<Secp256k1PrivateKey>,
    tx_fee_opt: Option<u64>,
) -> Option<Txid> {
    let (name, namespace) = split_fqn(&wrbsite_name).unwrap_or_else(|e| {
        eprintln!("FATAL: could not decode '{}': {}", &wrbsite_name, &e);
        process::exit(1);
    });

    let mut stackerdb_session =
        open_replica_stackerdb_session(contract_id.clone()).unwrap_or_else(|e| {
            eprintln!(
                "FATAL: failed to connect to StackerDB {} on replica node: {}",
                &contract_id, &e
            );
            process::exit(1);
        });

    let slots_metadata = stackerdb_session.list_chunks().unwrap_or_else(|e| {
        eprintln!(
            "FATAL: failed to list chunks on StackerDB {} on replica node {}: {}",
            &contract_id,
            &stackerdb_session.get_host(),
            &e
        );
        process::exit(1);
    });

    let slot_version = slots_metadata
        .get(slot_id as usize)
        .map(|slot_md| slot_md.slot_version)
        .unwrap_or_else(|| {
            eprintln!(
                "FATAL: no such StackerDB slot {} in {}",
                slot_id, &contract_id
            );
            process::exit(1);
        });

    let code_bytes = stackerdb_session
        .get_chunks(&[(slot_id, slot_version)])
        .unwrap_or_else(|e| {
            eprintln!("FATAL: failed to get site code chunk: {:?}", &e);
            process::exit(1);
        })
        .pop()
        .expect("FATAL(BUG): no slot value returned")
        .unwrap_or_else(|| {
            eprintln!(
                "FATAL: no code for slot {} in StackerDB {}",
                slot_id, &contract_id
            );
            process::exit(1);
        });

    // reconstruct the chunk
    let (privkey, mainnet) = with_global_config(|cfg| (cfg.private_key().clone(), cfg.mainnet()))
        .expect("System is not initialized");

    let name_privkey = name_privkey_opt.unwrap_or(privkey.clone());

    let mut chunk_data = StackerDBChunkData::new(slot_id, slot_version, code_bytes);
    chunk_data.sign(&privkey).unwrap_or_else(|e| {
        eprintln!("FATAL: failed to sign chunk: {:?}", &e);
        process::exit(1);
    });

    let metadata = chunk_data.get_slot_metadata();

    let mut zonefile = subcommand_bns_resolve(&wrbsite_name)
        .unwrap_or(format!("$ORIGIN {}\n\n", &wrbsite_name).as_bytes().to_vec());

    let mut wrb_rr_bytes = ZonefileResourceRecord::try_from(WrbTxtRecord::V1(WrbTxtRecordV1::new(
        contract_id.clone(),
        metadata,
    )))
    .expect("FATAL: could not construct a zonefile resource record for this wrbsite")
    .to_string()
    .as_bytes()
    .to_vec();

    zonefile.append(&mut "\n".as_bytes().to_vec());
    zonefile.append(&mut wrb_rr_bytes);
    if zonefile.len() > 8192 {
        eprintln!(
            "FATAL: new zonefile is too big (exceeds 8192 bytes)\nZonefile:\n{}",
            String::from_utf8_lossy(&zonefile)
        );
        process::exit(1);
    }

    if dry_run {
        if raw {
            let zonefile_hex = to_hex(&zonefile);
            println!("{}", &zonefile_hex);
        } else {
            println!("{}", &String::from_utf8_lossy(&zonefile));
        }
        return None;
    }

    let addr = StacksAddress::p2pkh(mainnet, &StacksPublicKey::from_private(&name_privkey))
        .to_account_principal();
    let mut runner = make_runner();
    let account = runner.get_account(&addr).unwrap_or_else(|e| {
        panic!("FATAL: failed to look up account {}: {:?}", &addr, &e);
    });

    let bns_address = StacksAddress::new(
        runner.get_bns_contract_id().issuer.version(),
        Hash160(runner.get_bns_contract_id().issuer.1.clone()),
    )
    .expect("Infallible");

    let bns_owner_opt = subcommand_bns_owner(&wrbsite_name);
    let Some(bns_owner) = bns_owner_opt else {
        eprintln!("FATAL: name '{}' is not registered", &wrbsite_name);
        process::exit(1);
    };

    if bns_owner.owner != addr {
        eprintln!(
            "FATAL: name '{}' is not owned by {}",
            &wrbsite_name,
            &addr.to_string()
        );
        process::exit(1);
    }

    let tx = make_tx(&mut runner, tx_fee_opt, |fee_rate| {
        make_contract_call(
            mainnet,
            &name_privkey,
            account.nonce,
            fee_rate,
            &bns_address,
            "zonefile-resolver",
            "update-zonefile",
            &[
                Value::buff_from(name.as_bytes().to_vec())
                    .expect("FATAL: name could not be converted to a buffer"),
                Value::buff_from(namespace.as_bytes().to_vec())
                    .expect("FATAL: namespace could not be converted to a buffer"),
                Value::some(
                    Value::buff_from(zonefile.clone())
                        .expect("FATAL: could not convert zonefile to buffer"),
                )
                .expect("FATAL: could not create (some zonefile)"),
            ],
            TransactionPostConditionMode::Deny,
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

    let txid = post_tx(&mut runner, &tx).unwrap_or_else(|e| {
        wrb_debug!("{}", &to_hex(&tx.serialize_to_vec()));
        eprintln!("FATAL: failed to post zonefile-update transaction: {}", &e);
        process::exit(1);
    });

    Some(txid)
}

/// site subcommand helper
/// Commands start at argv[2]
pub fn subcommand_site(mut argv: Vec<String>) {
    if argv.len() < 3 {
        eprintln!("Usage: {} site [subcommand] [options]", &argv[0]);
        process::exit(1);
    }
    let cmd = argv[2].clone();
    if cmd == "upload" {
        if argv.len() < 6 {
            eprintln!(
                "Usage: {} site {} WRBPOD_CONTRACT_ID SLOT_ID PATH_TO_SITE_CODE",
                &argv[0], &cmd
            );
            process::exit(1);
        }

        let contract_id = QualifiedContractIdentifier::parse(&argv[3]).unwrap_or_else(|e| {
            eprintln!("FATAL: invalid contract ID '{}': {:?}", &argv[3], &e);
            process::exit(1);
        });

        let slot_id = argv[4].parse::<u32>().unwrap_or_else(|e| {
            eprintln!(
                "FATAL: could not parse '{}' into slot ID: {:?}",
                &argv[4], &e
            );
            process::exit(1);
        });

        let path_to_code = &argv[5];
        let ack = subcommand_site_upload(&contract_id, slot_id, path_to_code.clone());
        if !ack.accepted {
            println!("{:?}", &ack);
            process::exit(1);
        }
        return;
    } else if cmd == "download" {
        if argv.len() < 5 {
            eprintln!(
                "Usage: {} site {} WRBPOD_CONTRACT_ID SLOT_ID",
                &argv[0], &cmd
            );
            process::exit(1);
        }

        let contract_id = QualifiedContractIdentifier::parse(&argv[3]).unwrap_or_else(|e| {
            eprintln!("FATAL: invalid contract ID '{}': {:?}", &argv[3], &e);
            process::exit(1);
        });

        let slot_id = argv[4].parse::<u32>().unwrap_or_else(|e| {
            eprintln!(
                "FATAL: could not parse '{}' into slot ID: {:?}",
                &argv[4], &e
            );
            process::exit(1);
        });

        let code = subcommand_site_download(&contract_id, slot_id);
        println!("{}", String::from_utf8_lossy(&code));
        return;
    } else if cmd == "publish" {
        if argv.len() < 6 {
            eprintln!("Usage: {} site {} [-n|--dry-run] [-r|--raw-hex] [-k|--name-private-key KEY] [-f|--fee FEE] WRBPOD_CONTRACT_ID SLOT_ID WRBSITE_NAME", &argv[0], &cmd);
            process::exit(1);
        }
        let dry_run = consume_arg(&mut argv, &["-n", "--dry-run"], false)
            .map_err(|e| {
                usage(&e);
                unreachable!()
            })
            .unwrap();

        let raw = consume_arg(&mut argv, &["-r", "--raw-hex"], false)
            .map_err(|e| {
                usage(&e);
                unreachable!()
            })
            .unwrap();

        let name_privkey_opt = consume_private_key(&mut argv, &["-k", "--private-key"]);
        let tx_fee_opt = consume_u64(&mut argv, &["-f", "--fee"]);

        let contract_id = QualifiedContractIdentifier::parse(&argv[3]).unwrap_or_else(|e| {
            eprintln!("FATAL: invalid contract ID '{}': {:?}", &argv[3], &e);
            process::exit(1);
        });

        let slot_id = argv[4].parse::<u32>().unwrap_or_else(|e| {
            eprintln!(
                "FATAL: could not parse '{}' into slot ID: {:?}",
                &argv[4], &e
            );
            process::exit(1);
        });

        let wrbsite_name = argv[5].clone();
        let txid_opt = subcommand_site_publish(
            &contract_id,
            slot_id,
            wrbsite_name,
            dry_run.is_some(),
            raw.is_some(),
            name_privkey_opt,
            tx_fee_opt,
        );
        if let Some(txid) = txid_opt {
            println!("{}", &txid);
        }

        return;
    } else if cmd == "deploy" {
        // uplaod and publish
        if argv.len() < 7 {
            eprintln!("Usage: {} site {} [-n|--dry-run] [-r|--raw-hex] [-k|--name-private-key KEY] [-f|--fee FEE] WRBPOD_CONTRACT_ID SLOT_ID WRBSITE_NAME PATH_TO_CODE", &argv[0], &cmd);
            process::exit(1);
        }
        let dry_run = consume_arg(&mut argv, &["-n", "--dry-run"], false)
            .map_err(|e| {
                usage(&e);
                unreachable!()
            })
            .unwrap();

        let raw = consume_arg(&mut argv, &["-r", "--raw-hex"], false)
            .map_err(|e| {
                usage(&e);
                unreachable!()
            })
            .unwrap();

        let name_privkey_opt = consume_private_key(&mut argv, &["-k", "--private-key"]);
        let tx_fee_opt = consume_u64(&mut argv, &["-f", "--fee"]);

        let contract_id = QualifiedContractIdentifier::parse(&argv[3]).unwrap_or_else(|e| {
            eprintln!("FATAL: invalid contract ID '{}': {:?}", &argv[3], &e);
            process::exit(1);
        });

        let slot_id = argv[4].parse::<u32>().unwrap_or_else(|e| {
            eprintln!(
                "FATAL: could not parse '{}' into slot ID: {:?}",
                &argv[4], &e
            );
            process::exit(1);
        });

        let wrbsite_name = argv[5].clone();
        let path_to_code = &argv[6];

        if !dry_run.is_some() {
            let ack = subcommand_site_upload(&contract_id, slot_id, path_to_code.clone());
            if !ack.accepted {
                eprintln!("{:?}", &ack);
                process::exit(1);
            }
        }
        let txid_opt = subcommand_site_publish(
            &contract_id,
            slot_id,
            wrbsite_name,
            dry_run.is_some(),
            raw.is_some(),
            name_privkey_opt,
            tx_fee_opt,
        );
        if let Some(txid) = txid_opt {
            println!("{}", &txid);
        }

        return;
    } else {
        usage("Unrecognized subcommand");
        unreachable!();
    }
}
