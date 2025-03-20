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

use crate::storage::mock::{LocalStackerDBClient, LocalStackerDBConfig};
use crate::storage::StackerDBClient;
use crate::storage::Wrbpod;
use crate::storage::WrbpodAddress;
use crate::storage::WrbpodSlices;
use crate::storage::WrbpodSuperblock;

use crate::core::globals::redirect_logfile;
use crate::core::with_global_config;
use crate::core::with_globals;

use crate::runner::stackerdb::StackerDBSession;

use crate::vm::clarity_vm::vm_execute;

use crate::tx::{
    make_contract_call, make_contract_publish, StacksTransaction, TransactionPostCondition,
    TransactionPostConditionMode,
};

use clarity::vm::types::QualifiedContractIdentifier;
use clarity::vm::types::StacksAddressExtensions;
use clarity::vm::types::StandardPrincipalData;
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
    consume_arg, consume_private_key, consume_u64, load_from_file_or_stdin, load_wrbsite_source,
    make_runner, make_tx, open_home_stackerdb_session, open_replica_stackerdb_session, post_tx,
    split_fqn, usage, wrbsite_load_code_bytes,
};

fn make_wrbpod_code(num_slots: u16, chunk_size: u32, write_freq: u32) -> String {
    let max_writes = u32::MAX;

    format!(
        r#"(define-constant OWNER tx-sender)
(define-constant NUM_SLOTS u{})
(define-constant CHUNK_SIZE u{})
(define-constant WRITE_FREQ u{})
(define-constant MAX_WRITES u{})
(define-constant MAX_NEIGHBORS u8)
(define-constant HINT_REPLICAS (list ))

(define-public (stackerdb-get-signer-slots)
    (ok (list {{ signer: OWNER, num-slots: NUM_SLOTS }})))

(define-public (stackerdb-get-config)
    (ok {{
        chunk-size: CHUNK_SIZE,
        write-freq: WRITE_FREQ,
        max-writes: MAX_WRITES,
        max-neighbors: MAX_NEIGHBORS,
        hint-replicas: HINT_REPLICAS
    }}))
"#,
        num_slots, chunk_size, write_freq, max_writes
    )
}

/// create a new wrbpod, blowing away anything that's already there, and open it to the given
/// session ID
fn wrbpod_format_session(wrbpod_addr: &WrbpodAddress) -> Result<(), String> {
    // already instantiated?
    if with_globals(|globals| {
        globals
            .get_wrbpod_session_id_by_address(wrbpod_addr)
            .is_some()
    }) {
        return Ok(());
    }

    // is this an owned wrbpod? only true if the client's identity private key matches the target
    // contract.
    let privkey = with_global_config(|cfg| cfg.private_key().clone())
        .ok_or("System is not initialized".to_string())?;

    // go set up the wrbpod session
    let mut runner = make_runner();
    let home_stackerdb_client = runner
        .get_home_stackerdb_client(wrbpod_addr.contract.clone(), privkey.clone())
        .map_err(|e| {
            format!(
                "Failed to instantiate StackerDB client to {}: {:?}",
                &wrbpod_addr.contract, &e
            )
        })?;

    let replica_stackerdb_client = runner
        .get_replica_stackerdb_client(wrbpod_addr.contract.clone(), privkey.clone())
        .map_err(|e| {
            format!(
                "Failed to instantiate StackerDB client to {}: {:?}",
                &wrbpod_addr.contract, &e
            )
        })?;

    let wrbpod_session = Wrbpod::format(
        home_stackerdb_client,
        replica_stackerdb_client,
        privkey.clone(),
        wrbpod_addr.slot,
    )
    .map_err(|e| {
        format!(
            "Failed to open wrbpod session to {}: {:?}",
            &wrbpod_addr.contract, &e
        )
    })?;

    with_globals(|globals| {
        let session_id = globals.next_wrbpod_session_id();
        globals.add_wrbpod_session(session_id, wrbpod_addr.clone(), wrbpod_session);
    });
    Ok(())
}

/// connect to an existing wrbpod and open it to the given session ID
pub fn wrbpod_open_session(wrbpod_addr: &WrbpodAddress) -> Result<(), String> {
    // already instantiated?
    if with_globals(|globals| {
        globals
            .get_wrbpod_session_id_by_address(wrbpod_addr)
            .is_some()
    }) {
        wrb_test_debug!("Session for {} is already opened", &wrbpod_addr);
        return Ok(());
    }

    // is this an owned wrbpod? only true if the client's identity private key matches the target
    // contract.
    let privkey = with_global_config(|cfg| cfg.private_key().clone())
        .ok_or("System is not initialized".to_string())?;

    // go set up the wrbpod session
    let mut runner = make_runner();
    let home_stackerdb_client = runner
        .get_home_stackerdb_client(wrbpod_addr.contract.clone(), privkey.clone())
        .map_err(|e| {
            format!(
                "Failed to instantiate StackerDB client to {}: {:?}",
                &wrbpod_addr.contract, &e
            )
        })?;

    let replica_stackerdb_client = runner
        .get_replica_stackerdb_client(wrbpod_addr.contract.clone(), privkey.clone())
        .map_err(|e| {
            format!(
                "Failed to instantiate StackerDB client to {}: {:?}",
                &wrbpod_addr.contract, &e
            )
        })?;

    let wrbpod_session = Wrbpod::open(
        home_stackerdb_client,
        replica_stackerdb_client,
        privkey.clone(),
        wrbpod_addr.slot,
    )
    .map_err(|e| {
        format!(
            "Failed to open wrbpod session to {}: {:?}",
            &wrbpod_addr.contract, &e
        )
    })?;

    with_globals(|globals| {
        let session_id = globals.next_wrbpod_session_id();
        globals.add_wrbpod_session(session_id, wrbpod_addr.clone(), wrbpod_session);
    });
    Ok(())
}

/// get the contract ID from either argv or from the config file
fn wrbpod_get_address(argv: &mut Vec<String>) -> Result<WrbpodAddress, String> {
    let wrbpod_addr_str = if let Some(addr_str) = consume_arg(argv, &["-w", "--wrbpod"], true)
        .map_err(|e| {
            usage(&e);
            unreachable!()
        })
        .unwrap()
    {
        addr_str
    } else {
        with_global_config(|cfg| cfg.default_wrbpod().to_string())
            .expect("System is not initialized")
    };
    let wrbpod_addr = WrbpodAddress::parse(&wrbpod_addr_str)
        .ok_or(format!("could not parse '{}'", &wrbpod_addr_str))?;

    Ok(wrbpod_addr)
}

/// get a wrbpod slot.
/// sets up wrbpod session 0
fn wrbpod_get_slot(
    wrbpod_addr: &WrbpodAddress,
    app_name: &str,
    app_slot_num: u32,
) -> Option<WrbpodSlices> {
    wrbpod_open_session(&wrbpod_addr)
        .map_err(|e| {
            eprintln!("FATAL: {}", &e);
            process::exit(1);
        })
        .unwrap();

    let slot = with_globals(|globals| {
        let wrbpod_session = globals.get_wrbpod_session_by_address(wrbpod_addr).unwrap();
        wrbpod_session
            .fetch_chunk(&app_name, app_slot_num)
            .map_err(|e| {
                eprintln!("FATAL: failed to fetch chunk: {:?}", &e);
                process::exit(1);
            })
            .unwrap();

        let chunk_ref_opt = wrbpod_session.ref_app_chunk(&app_name, app_slot_num);
        chunk_ref_opt.cloned()
    });

    slot
}

/// get wrbpod superblock
fn wrbpod_get_superblock(wrbpod_addr: &WrbpodAddress) -> WrbpodSuperblock {
    wrbpod_open_session(&wrbpod_addr)
        .map_err(|e| {
            eprintln!("FATAL: {}", &e);
            process::exit(1);
        })
        .unwrap();

    let superblock = with_globals(|globals| {
        let wrbpod_session = globals.get_wrbpod_session_by_address(wrbpod_addr).unwrap();
        let superblock = wrbpod_session.superblock().clone();
        superblock
    });

    superblock
}

/// allocate wrbpod slots for an app
fn wrbpod_alloc_slots(
    wrbpod_addr: &WrbpodAddress,
    app_name: &str,
    num_slots: u32,
    wrbsite_data_source_opt: Option<String>,
) -> bool {
    let (bytes, _) = load_wrbsite_source(&app_name, wrbsite_data_source_opt)
        .map_err(|e| {
            usage(&e);
            unreachable!()
        })
        .unwrap();

    let code_hash = Hash160::from_data(&bytes);

    wrbpod_open_session(wrbpod_addr)
        .map_err(|e| {
            eprintln!("FATAL: {}", &e);
            process::exit(1);
        })
        .unwrap();

    let res = with_globals(|globals| {
        let wrbpod_session = globals.get_wrbpod_session_by_address(wrbpod_addr).unwrap();
        wrbpod_session
            .allocate_slots(&app_name, code_hash, num_slots)
            .map_err(|e| {
                eprintln!("FATAL: failed to allocate slots: {:?}", &e);
                process::exit(1);
            })
            .unwrap()
    });
    res
}

/// put a wrbpod chunk for an app
fn wrbpod_put_chunk(
    wrbpod_addr: &WrbpodAddress,
    app_name: &str,
    app_slot_num: u32,
    app_slot_path: &str,
) {
    let slot: WrbpodSlices = if app_slot_path == "-" {
        let mut fd = stdin();
        serde_json::from_reader(&mut fd)
            .map_err(|e| {
                eprintln!("FATAL: failed to load app slot from stdin: {:?}", &e);
                process::exit(1);
            })
            .unwrap()
    } else if fs::metadata(&app_slot_path).is_ok() {
        // this is a file
        let data = fs::read(&app_slot_path)
            .map_err(|e| {
                eprintln!(
                    "FATAL: failed to read data from {}: {:?}",
                    &app_slot_path, &e
                );
                process::exit(1);
            })
            .unwrap();

        serde_json::from_slice(data.as_slice())
            .map_err(|e| {
                eprintln!(
                    "FATAL: failed to decode data from JSON in {}: {:?}",
                    &app_slot_path, &e
                );
                process::exit(1);
            })
            .unwrap()
    } else {
        // this may be raw JSON
        serde_json::from_reader(&mut app_slot_path.as_bytes())
            .map_err(|e| {
                eprintln!("FATAL: failed to load app slot from argument: {:?}", &e);
                process::exit(1);
            })
            .unwrap()
    };

    wrbpod_open_session(wrbpod_addr)
        .map_err(|e| {
            eprintln!("FATAL: {}", &e);
            process::exit(1);
        })
        .unwrap();

    let Some(stackerdb_chunk_id) = with_globals(|globals| {
        let wrbpod_session = globals.get_wrbpod_session_by_address(wrbpod_addr).unwrap();
        wrbpod_session.app_slot_id_to_stackerdb_chunk_id(&app_name, app_slot_num)
    }) else {
        eprintln!(
            "FATAL: app slot {} is not occupied by app '{}'",
            app_slot_num, app_name
        );
        process::exit(1);
    };

    let slots_metadata = with_globals(|globals| {
        let wrbpod_session = globals.get_wrbpod_session_by_address(wrbpod_addr).unwrap();
        wrbpod_session.list_chunks()
    })
    .map_err(|e| {
        eprintln!(
            "FATAL: failed to list chunks in {}: {:?}",
            &wrbpod_addr.contract, &e
        );
        process::exit(1);
    })
    .unwrap();

    let slot_version = slots_metadata
        .get(stackerdb_chunk_id as usize)
        .map(|slot_md| slot_md.slot_version)
        .unwrap_or_else(|| {
            eprintln!(
                "FATAL: no such StackerDB slot {} (app slot {}) in {}",
                stackerdb_chunk_id, app_slot_num, &wrbpod_addr.contract
            );
            process::exit(1);
        });

    let chunk_data = slot.to_stackerdb_chunk(stackerdb_chunk_id, slot_version + 1);

    with_globals(|globals| {
        let wrbpod_session = globals.get_wrbpod_session_by_address(wrbpod_addr).unwrap();
        wrbpod_session
            .put_chunk(chunk_data)
            .map_err(|e| {
                eprintln!(
                    "FATAL: failed to put app slot {} to '{}': {:?}",
                    &app_slot_num, &app_name, &e
                );
                process::exit(1);
            })
            .unwrap();
    });
}

/// clear a wrbpod slot
fn wrbpod_clear_slot(wrbpod_addr: &WrbpodAddress, app_name: &str, app_slot_num: u32) {
    let slot = WrbpodSlices::new();

    wrbpod_open_session(&wrbpod_addr)
        .map_err(|e| {
            eprintln!("FATAL: {}", &e);
            process::exit(1);
        })
        .unwrap();

    let Some(stackerdb_chunk_id) = with_globals(|globals| {
        let wrbpod_session = globals.get_wrbpod_session_by_address(wrbpod_addr).unwrap();
        wrbpod_session.app_slot_id_to_stackerdb_chunk_id(&app_name, app_slot_num)
    }) else {
        eprintln!(
            "FATAL: app slot {} is not occupied by app '{}'",
            app_slot_num, app_name
        );
        process::exit(1);
    };

    let slots_metadata = with_globals(|globals| {
        let wrbpod_session = globals.get_wrbpod_session_by_address(wrbpod_addr).unwrap();
        wrbpod_session.list_chunks()
    })
    .map_err(|e| {
        eprintln!(
            "FATAL: failed to list chunks in {}: {:?}",
            &wrbpod_addr.contract, &e
        );
        process::exit(1);
    })
    .unwrap();

    let slot_version = slots_metadata
        .get(stackerdb_chunk_id as usize)
        .map(|slot_md| slot_md.slot_version)
        .unwrap_or_else(|| {
            eprintln!(
                "FATAL: no such StackerDB slot {} (app slot {}) in {}",
                stackerdb_chunk_id, app_slot_num, &wrbpod_addr
            );
            process::exit(1);
        });

    let chunk_data = slot.to_stackerdb_chunk(stackerdb_chunk_id, slot_version + 1);

    with_globals(|globals| {
        let wrbpod_session = globals.get_wrbpod_session_by_address(wrbpod_addr).unwrap();
        wrbpod_session
            .put_chunk(chunk_data)
            .map_err(|e| {
                eprintln!(
                    "FATAL: failed to put app slot {} to '{}': {:?}",
                    &app_slot_num, &app_name, &e
                );
                process::exit(1);
            })
            .unwrap();
    });
}

/// put a wrbpod slite into a wrbpod slot
fn wrbpod_put_slice(
    wrbpod_addr: &WrbpodAddress,
    app_name: &str,
    app_slot_num: u32,
    app_slice_id: u128,
    app_slice_bytes: Vec<u8>,
) {
    // this must be a Clarity value
    let _app_clarity_value = Value::consensus_deserialize(&mut &app_slice_bytes[..])
        .map_err(|e| {
            eprintln!("FATAL: not a Clarity value: {:?}", &e);
            process::exit(1);
        })
        .unwrap();

    wrbpod_open_session(&wrbpod_addr)
        .map_err(|e| {
            eprintln!("FATAL: {}", &e);
            process::exit(1);
        })
        .unwrap();

    with_globals(|globals| {
        let wrbpod_session = globals.get_wrbpod_session_by_address(wrbpod_addr).unwrap();
        let len = app_slice_bytes.len();
        if !wrbpod_session.put_slice(&app_name, app_slot_num, app_slice_id, app_slice_bytes) {
            eprintln!("FATAL: failed to store slice of {} bytes. Either the app slot is not mapped, or the resulting slot would be too big", len);
            process::exit(1);
        }
        wrbpod_session
            .sync_slot(&app_name, app_slot_num)
            .map_err(|e| {
                eprintln!("FATAL: failed to save app slot: {:?}", &e);
                process::exit(1);
            })
            .unwrap();
    });
}

/// deploy a wrbpod
fn wrbpod_deploy(
    name: &str,
    num_slots: u16,
    slot_size: u32,
    write_freq: u32,
    dry_run: bool,
    tx_fee_opt: Option<u64>,
    privkey_opt: Option<Secp256k1PrivateKey>,
) -> Option<Txid> {
    let (privkey, mainnet) = with_global_config(|cfg| (cfg.private_key().clone(), cfg.mainnet()))
        .expect("System is not initialized");

    let privkey = privkey_opt.unwrap_or(privkey);

    let code = make_wrbpod_code(num_slots, slot_size, write_freq);
    let mut runner = make_runner();

    let stacks_addr = StacksAddress::p2pkh(mainnet, &StacksPublicKey::from_private(&privkey));
    let principal =
        StandardPrincipalData::new(stacks_addr.version(), stacks_addr.bytes().clone().0).unwrap();
    let account = runner
        .get_account(&principal.clone().into())
        .unwrap_or_else(|e| {
            panic!("FATAL: failed to look up account {}: {:?}", &principal, &e);
        });

    let tx = make_tx(&mut runner, tx_fee_opt, |fee_rate| {
        make_contract_publish(
            mainnet,
            &privkey,
            account.nonce,
            fee_rate,
            name,
            &code,
            TransactionPostConditionMode::Deny,
            vec![],
        )
        .expect("FATAL: could not make wrbpod transaction")
    })
    .unwrap_or_else(|e| {
        eprintln!("FATAL: failed to generate wrbpod transaction: {}", &e);
        process::exit(1);
    });

    if dry_run {
        println!("{}", &to_hex(&tx.serialize_to_vec()));
        return None;
    }

    let txid = post_tx(&mut runner, &tx).unwrap_or_else(|e| {
        wrb_debug!("{}", &to_hex(&tx.serialize_to_vec()));
        eprintln!("FATAL: failed to post wrbpod transaction: {}", &e);
        process::exit(1);
    });

    Some(txid)
}

/// Instantiate a mocked stackerdb
fn wrbpod_mock_stackerdb(path: &str, config: LocalStackerDBConfig) {
    if std::fs::metadata(path).is_ok() {
        let _ = std::fs::remove_file(path).expect(&format!("FATAL: could not remove '{}'", path));
    }

    let _ = LocalStackerDBClient::open_or_create(path, config)
        .expect("Failed to instantiate mocked StackerDB");
}

/// wrbpod subcommand helper
/// Commands start at argv[2]
pub fn subcommand_wrbpod(mut argv: Vec<String>, wrbsite_data_source_opt: Option<String>) {
    if argv.len() < 3 {
        eprintln!("Usage: {} wrbpod [subcommand] [options]", &argv[0]);
        process::exit(1);
    }
    wrb_debug!("Wrbpod subcommand args: '{:?}'", &argv);

    let cmd = argv[2].clone();
    if cmd == "format" {
        if argv.len() < 3 {
            eprintln!(
                "Usage: {} wrbpod {} [-w wrbpod_wrbpod_addr]",
                &argv[0], &cmd
            );
            process::exit(1);
        }
        let wrbpod_addr = wrbpod_get_address(&mut argv)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        wrbpod_format_session(&wrbpod_addr)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        return;
    } else if cmd == "get-superblock" {
        if argv.len() < 3 {
            eprintln!(
                "Usage: {} wrbpod {} [-w wrbpod_wrbpod_addr]",
                &argv[0], &cmd
            );
            process::exit(1);
        }
        let wrbpod_addr = wrbpod_get_address(&mut argv)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        let superblock = wrbpod_get_superblock(&wrbpod_addr);

        println!(
            "{}",
            serde_json::to_string(&superblock)
                .map_err(|e| {
                    eprintln!("FATAL: failed to serialize superblock to JSON: {:?}", &e);
                    process::exit(1)
                })
                .unwrap()
        );

        return;
    } else if cmd == "alloc-slots" {
        if argv.len() < 5 {
            eprintln!(
                "Usage: {} wrbpod {} [-w wrbpod_wrbpod_addr] [-s app_source] APP_NAME NUM_SLOTS",
                &argv[0], &cmd
            );
            process::exit(1);
        }
        let wrbpod_addr = wrbpod_get_address(&mut argv)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        let app_name = argv[3].clone();
        let num_slots: u32 = argv[4]
            .parse()
            .map_err(|_e| {
                eprintln!("FATAL: could not parse '{}' into a u32", &argv[4]);
                process::exit(1);
            })
            .unwrap();

        let res = wrbpod_alloc_slots(&wrbpod_addr, &app_name, num_slots, wrbsite_data_source_opt);

        if !res {
            eprintln!("Failed to allocate new slots");
            process::exit(1);
        }

        return;
    } else if cmd == "put-slot" {
        if argv.len() < 6 {
            eprintln!("Usage: {} wrbpod {} [-w wrbpod_wrbpod_addr] APP_NAME APP_SLOT_NUM APP_SLOT_JSON_OR_STDIN", &argv[0], &cmd);
            process::exit(1);
        }
        let wrbpod_addr = wrbpod_get_address(&mut argv)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        let app_name = argv[3].clone();
        let app_slot_num: u32 = argv[4]
            .parse()
            .map_err(|_e| {
                eprintln!("FATAL: could not parse '{}' into u32", &argv[4]);
                process::exit(1);
            })
            .unwrap();
        let app_slot_path = argv[5].clone();

        wrbpod_put_chunk(&wrbpod_addr, &app_name, app_slot_num, &app_slot_path);
        return;
    } else if cmd == "clear-slot" {
        if argv.len() < 6 {
            eprintln!(
                "Usage: {} wrbpod {} [-w wrbpod_wrbpod_addr] APP_NAME APP_SLOT_NUM",
                &argv[0], &cmd
            );
            process::exit(1);
        }
        let wrbpod_addr = wrbpod_get_address(&mut argv)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        let app_name = argv[3].clone();
        let app_slot_num: u32 = argv[4]
            .parse()
            .map_err(|_e| {
                eprintln!("FATAL: could not parse '{}' into u32", &argv[4]);
                process::exit(1);
            })
            .unwrap();

        wrbpod_clear_slot(&wrbpod_addr, &app_name, app_slot_num);
        return;
    } else if cmd == "get-chunk" {
        if argv.len() < 4 {
            eprintln!(
                "Usage: {} wrbpod {} [-w wrbpod_wrbpod_addr] SLOT_NUM",
                &argv[0], &cmd
            );
            process::exit(1);
        }
        let wrbpod_addr = wrbpod_get_address(&mut argv)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        let slot_num: u32 = argv[3]
            .parse()
            .map_err(|_e| {
                eprintln!("FATAL: could not parse '{}' into u32", &argv[4]);
                process::exit(1);
            })
            .unwrap();

        wrbpod_open_session(&wrbpod_addr)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        let slot = with_globals(|globals| {
            let wrbpod_session = globals.get_wrbpod_session_by_address(&wrbpod_addr).unwrap();
            let chunk_data_opt = wrbpod_session
                .get_and_verify_raw_chunk(slot_num)
                .map_err(|e| {
                    eprintln!("FATAL: failed to fetch chunk: {:?}", &e);
                    process::exit(1);
                })
                .unwrap();

            chunk_data_opt
        });

        if let Some(slot) = slot {
            println!("{}", &to_hex(&slot));
        } else {
            eprintln!("No such slot");
        }
        return;
    } else if cmd == "get-slot" {
        if argv.len() < 5 {
            eprintln!(
                "Usage: {} wrbpod get-slot [-w wrbpod_wrbpod_addr] APP_NAME APP_SLOT_NUM",
                &argv[0]
            );
            process::exit(1);
        }
        let wrbpod_addr = wrbpod_get_address(&mut argv)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        let app_name = argv[3].clone();
        let app_slot_num: u32 = argv[4]
            .parse()
            .map_err(|_e| {
                eprintln!("FATAL: could not parse '{}' into u32", &argv[4]);
                process::exit(1);
            })
            .unwrap();

        let slot_opt = wrbpod_get_slot(&wrbpod_addr, &app_name, app_slot_num);
        if let Some(slot) = slot_opt {
            println!("{}", &serde_json::to_string(&slot).unwrap());
        } else {
            eprintln!("No such slot");
        }
        return;
    } else if cmd == "get-slice" {
        if argv.len() < 7 {
            eprintln!(
                "Usage: {} wrbpod {} [-w wrbpod_wrbpod_addr] APP_NAME APP_SLOT_NUM APP_SLICE_ID",
                &argv[0], &cmd
            );
            process::exit(1);
        }
        let wrbpod_addr = wrbpod_get_address(&mut argv)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        let app_name = argv[3].clone();
        let app_slot_num: u32 = argv[4]
            .parse()
            .map_err(|_e| {
                eprintln!("FATAL: could not parse '{}' into u32", &argv[4]);
                process::exit(1);
            })
            .unwrap();

        let app_slice_id: u128 = argv[5]
            .parse()
            .map_err(|_e| {
                eprintln!("FATAL: could not parse '{}' into u128", &argv[4]);
                process::exit(1);
            })
            .unwrap();

        let slot_opt = wrbpod_get_slot(&wrbpod_addr, &app_name, app_slot_num);
        if let Some(slot) = slot_opt {
            if let Some(slice) = slot.get_slice(app_slice_id) {
                println!("{}", &to_hex(slice));
            } else {
                eprintln!("No such slice");
            }
        } else {
            eprintln!("No such slot");
        }
        return;
    } else if cmd == "put-slice" {
        if argv.len() < 7 {
            eprintln!("Usage: {} wrbpod {} [-w wrbpod_wrbpod_addr] APP_NAME APP_SLOT_NUM APP_SLICE_ID APP_SLICE_DATA", &argv[0], &cmd);
            process::exit(1);
        }
        let wrbpod_addr = wrbpod_get_address(&mut argv)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        let app_name = argv[3].clone();
        let app_slot_num: u32 = argv[4]
            .parse()
            .map_err(|_e| {
                eprintln!("FATAL: could not parse '{}' into u32", &argv[4]);
                process::exit(1);
            })
            .unwrap();

        let app_slice_id: u128 = argv[5]
            .parse()
            .map_err(|_e| {
                eprintln!("FATAL: could not parse '{}' into u128", &argv[4]);
                process::exit(1);
            })
            .unwrap();

        let app_slice_data = argv[6].clone();
        let app_slice_bytes = hex_bytes(&app_slice_data)
            .map_err(|_e| {
                eprintln!("FATAL: could not decode hex string '{}'", &app_slice_data);
                process::exit(1);
            })
            .unwrap();

        // this must be a Clarity value
        let _app_clarity_value = Value::consensus_deserialize(&mut &app_slice_bytes[..])
            .map_err(|e| {
                eprintln!("FATAL: not a Clarity value: {:?}", &e);
                process::exit(1);
            })
            .unwrap();

        wrbpod_put_slice(
            &wrbpod_addr,
            &app_name,
            app_slot_num,
            app_slice_id,
            app_slice_bytes,
        );
        return;
    } else if cmd == "deploy" {
        if argv.len() < 6 {
            eprintln!("Usage: {} wrbpod {} [-n|--dry-run] [-k|--private-key KEY] [-f|--fee FEE] CONTRACT_NAME SLOT_SIZE NUM_SLOTS [WRITE_FREQ]", &argv[0], &cmd);
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

        let contract_name = argv[3].clone();
        let num_slots = argv[4]
            .parse::<u16>()
            .expect("FATAL: num_slots is not a u16");
        if num_slots > 4096 {
            panic!("Wrbpods cannot be more than 4096 slots");
        }

        let slot_size = argv[5]
            .parse::<u32>()
            .expect("FATAL: slot_size is not a u32");
        if slot_size > 1024 * 1024 * 2 {
            panic!("Wrbpod slots cannot be bigger than 2MB");
        }

        let write_freq = if argv.len() >= 7 {
            argv[6]
                .parse::<u32>()
                .expect("FATAL: write_freq is not a u32")
        } else {
            10
        };

        let txid_opt = wrbpod_deploy(
            &contract_name,
            num_slots,
            slot_size,
            write_freq,
            dry_run.is_some(),
            tx_fee_opt,
            privkey_opt,
        );
        let Some(txid) = txid_opt else {
            eprintln!("Contract already exists");
            process::exit(1);
        };
        println!("{}", &txid);
        return;
    } else if cmd == "mock-stackerdb" {
        if argv.len() < 5 {
            eprintln!(
                "Usage: {} wrbpod {} /path/to/config.json /path/to/db.sqlite",
                &argv[0], &cmd
            );
            process::exit(1);
        }

        let config_json = load_from_file_or_stdin(&argv[3]);
        let config: LocalStackerDBConfig =
            serde_json::from_slice(&config_json).unwrap_or_else(|e| {
                panic!("FATAL: could not decode config: {:?}", &e);
            });

        wrbpod_mock_stackerdb(&argv[4], config);
        eprintln!("Database created");
        return;
    }

    eprintln!("Unrecognized `wrbpod` command '{}'", &cmd);
    process::exit(1);
}
