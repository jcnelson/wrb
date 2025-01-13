#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

extern crate stacks_common;

#[macro_use]
extern crate serde_json;

#[macro_use]
extern crate clarity;

#[macro_use]
extern crate lazy_static;

extern crate libstackerdb;

extern crate dirs;
extern crate lzma_rs;
extern crate rusqlite;
extern crate termion;

#[macro_use]
pub mod util;

pub mod core;
pub mod runner;
pub mod storage;
pub mod ui;
pub mod viewer;
pub mod vm;

use std::env;
use std::fs;
use std::io::{stdin, stdout, Read};
use std::path::Path;
use std::process;
use std::thread;

use crate::core::Config;
use crate::runner::Runner;
use crate::ui::events::WrbChannels;
use crate::ui::events::WrbEvent;
use crate::ui::Renderer;
use crate::viewer::Viewer;
use crate::vm::ClarityVM;

use crate::util::privkey_to_principal;
use crate::util::{DEFAULT_WRB_CLARITY_VERSION, DEFAULT_WRB_EPOCH};

use crate::storage::Wrbpod;
use crate::storage::WrbpodSlices;
use crate::storage::WrbpodSuperblock;

use crate::core::globals::redirect_logfile;
use crate::core::with_global_config;
use crate::core::with_globals;

use crate::vm::clarity_vm::vm_execute;

use clarity::vm::types::QualifiedContractIdentifier;
use clarity::vm::types::TupleData;
use clarity::vm::ClarityName;
use clarity::vm::Value;

use crate::vm::special::{get_home_stackerdb_client, get_replica_stackerdb_client};

use stacks_common::address::{
    C32_ADDRESS_VERSION_MAINNET_SINGLESIG, C32_ADDRESS_VERSION_TESTNET_SINGLESIG,
};
use stacks_common::types::chainstate::StacksAddress;
use stacks_common::util::hash::hex_bytes;
use stacks_common::util::hash::to_hex;
use stacks_common::util::hash::Hash160;

use crate::stacks_common::codec::StacksMessageCodec;

const DEFAULT_CONFIG: &str = ".wrb/config.toml";

fn consume_arg(
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

fn usage(msg: &str) {
    let args: Vec<_> = env::args().collect();
    eprintln!("FATAL: {}", msg);
    eprintln!("Usage: {} [options] wrbsite", &args[0]);
    process::exit(1);
}

/// Load the wrbsite for the given name from the given source
fn load_wrbsite_source(wrbsite_name: &str, source: Option<String>) -> Result<Vec<u8>, String> {
    let Some(path) = source else {
        let mut wrbsite_split = wrbsite_name.split(".");
        let Some(name) = wrbsite_split.next() else {
            return Err("Malformed wrbsite name -- no '.'".to_string());
        };
        let Some(namespace) = wrbsite_split.next() else {
            return Err("Malformed wrbsite name -- no namespace".to_string());
        };

        let (node_host, node_port) =
            with_global_config(|cfg| cfg.get_node_addr()).expect("FATAL: system not initialized");
        let bns_contract_id = with_global_config(|cfg| cfg.get_bns_contract_id())
            .expect("FATAL: system not initialized");
        let mut runner = Runner::new(bns_contract_id, node_host, node_port);
        let bns_res = runner
            .bns_lookup(namespace, name)
            .map_err(|e| format!("Failed to look up '{}': {:?}", wrbsite_name, &e))?;

        let bns_rec = bns_res
            .map_err(|e| format!("BNS error when looking up '{}': {:?}", wrbsite_name, &e))?;

        let Some(zonefile) = bns_rec.zonefile else {
            return Err(format!("Name '{}' has no zonefile data", &wrbsite_name));
        };

        return Ok(zonefile);
    };

    // treat source as a path to uncompressed clarity code
    let code = fs::read_to_string(&path).map_err(|e| format!("Invalid path: {}", &e))?;
    let bytes = Renderer::encode_bytes(code.as_bytes())
        .map_err(|e| format!("Failed to encode source code from '{}': {:?}", &path, &e))?;

    Ok(bytes)
}

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
fn json_to_clarity<R: Read>(fd: &mut R) -> Result<Value, String> {
    let json_obj: serde_json::Value =
        serde_json::from_reader(fd).map_err(|e| format!("Failed to decode JSON: {:?}", &e))?;

    inner_json_to_clarity(json_obj)
}

/// clarity subcommand handler.
/// Commands start at argv[2]
fn subcommand_clarity(argv: Vec<String>) {
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

/// create a new wrbpod, blowing away anything that's already there, and open it to the given
/// session ID
fn format_wrbpod_session(
    contract_addr: &QualifiedContractIdentifier,
    session_id: u128,
) -> Result<(), String> {
    // already instantiated?
    if with_globals(|globals| globals.get_wrbpod_session(session_id).is_some()) {
        return Ok(());
    }

    // is this an owned wrbpod? only true if the client's identity private key matches the target
    // contract.
    let privkey = with_global_config(|cfg| cfg.private_key().clone())
        .ok_or("System is not initialized".to_string())?;

    // go set up the wrbpod session
    let (node_host, node_port) =
        with_global_config(|cfg| cfg.get_node_addr()).expect("FATAL: system not initialized");
    let bns_contract_id =
        with_global_config(|cfg| cfg.get_bns_contract_id()).expect("FATAL: system not initialized");
    let mut runner = Runner::new(bns_contract_id, node_host, node_port);
    let home_stackerdb_client =
        get_home_stackerdb_client(&mut runner, contract_addr.clone(), privkey.clone()).map_err(
            |e| {
                format!(
                    "Failed to instantiate StackerDB client to {}: {:?}",
                    contract_addr, &e
                )
            },
        )?;

    let replica_stackerdb_client =
        get_replica_stackerdb_client(&mut runner, contract_addr.clone(), privkey.clone()).map_err(
            |e| {
                format!(
                    "Failed to instantiate StackerDB client to {}: {:?}",
                    contract_addr, &e
                )
            },
        )?;

    let wrbpod_session = Wrbpod::format(
        home_stackerdb_client,
        replica_stackerdb_client,
        privkey.clone(),
    )
    .map_err(|e| {
        format!(
            "Failed to open wrbpod session to {}: {:?}",
            contract_addr, &e
        )
    })?;

    with_globals(|globals| {
        globals.add_wrbpod_session(session_id, wrbpod_session);
    });
    Ok(())
}

/// connect to an existing wrbpod and open it to the given session ID
fn setup_wrbpod_session(
    contract_addr: &QualifiedContractIdentifier,
    session_id: u128,
) -> Result<(), String> {
    // already instantiated?
    if with_globals(|globals| globals.get_wrbpod_session(session_id).is_some()) {
        return Ok(());
    }

    // is this an owned wrbpod? only true if the client's identity private key matches the target
    // contract.
    let privkey = with_global_config(|cfg| cfg.private_key().clone())
        .ok_or("System is not initialized".to_string())?;

    // go set up the wrbpod session
    let (node_host, node_port) =
        with_global_config(|cfg| cfg.get_node_addr()).expect("FATAL: system not initialized");
    let bns_contract_id =
        with_global_config(|cfg| cfg.get_bns_contract_id()).expect("FATAL: system not initialized");
    let mut runner = Runner::new(bns_contract_id, node_host, node_port);
    let home_stackerdb_client =
        get_home_stackerdb_client(&mut runner, contract_addr.clone(), privkey.clone()).map_err(
            |e| {
                format!(
                    "Failed to instantiate StackerDB client to {}: {:?}",
                    contract_addr, &e
                )
            },
        )?;

    let replica_stackerdb_client =
        get_replica_stackerdb_client(&mut runner, contract_addr.clone(), privkey.clone()).map_err(
            |e| {
                format!(
                    "Failed to instantiate StackerDB client to {}: {:?}",
                    contract_addr, &e
                )
            },
        )?;

    let wrbpod_session = Wrbpod::open(
        home_stackerdb_client,
        replica_stackerdb_client,
        privkey.clone(),
    )
    .map_err(|e| {
        format!(
            "Failed to open wrbpod session to {}: {:?}",
            contract_addr, &e
        )
    })?;

    with_globals(|globals| {
        globals.add_wrbpod_session(session_id, wrbpod_session);
    });
    Ok(())
}

/// get the contract ID from either argv or from the config file
fn get_wrbpod_contract_id(argv: &mut Vec<String>) -> Result<QualifiedContractIdentifier, String> {
    let contract_addr_str = if let Some(addr_str) = consume_arg(argv, &["-w", "--wrbpod"], true)
        .map_err(|e| {
            usage(&e);
            unreachable!()
        })
        .unwrap()
    {
        addr_str
    } else {
        // deduce from private key
        let (privkey, mainnet) =
            with_global_config(|cfg| (cfg.private_key().clone(), cfg.mainnet()))
                .ok_or(format!("System is not initialized"))?;

        let principal = privkey_to_principal(
            &privkey,
            if mainnet {
                C32_ADDRESS_VERSION_MAINNET_SINGLESIG
            } else {
                C32_ADDRESS_VERSION_TESTNET_SINGLESIG
            },
        );
        let addr = StacksAddress {
            version: principal.0,
            bytes: Hash160(principal.1),
        };
        let addr_str = format!("{}.wrbpod", &addr);
        addr_str
    };
    let contract_addr = QualifiedContractIdentifier::parse(&contract_addr_str)
        .map_err(|_e| format!("could not parse '{}'", &contract_addr_str))?;

    Ok(contract_addr)
}

/// get a wrbpod slot.
/// sets up wrbpod session 0
fn get_wrbpod_slot(
    contract_addr: &QualifiedContractIdentifier,
    app_name: &str,
    app_slot_num: u32,
) -> Result<Option<WrbpodSlices>, String> {
    setup_wrbpod_session(&contract_addr, 0)
        .map_err(|e| {
            eprintln!("FATAL: {}", &e);
            process::exit(1);
        })
        .unwrap();

    let slot = with_globals(|globals| {
        let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
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
    Ok(slot)
}

/// wrbpod subcommand helper
/// Commands start at argv[2]
fn subcommand_wrbpod(mut argv: Vec<String>, wrbsite_data_source_opt: Option<String>) {
    if argv.len() < 3 {
        eprintln!("Usage: {} wrbpod [subcommand] [options]", &argv[0]);
        process::exit(1);
    }

    let cmd = argv[2].clone();
    if cmd == "format" {
        if argv.len() < 3 {
            eprintln!(
                "Usage: {} wrbpod {} [-w wrbpod_contract_addr]",
                &argv[0], &cmd
            );
            process::exit(1);
        }
        let contract_addr = get_wrbpod_contract_id(&mut argv)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        format_wrbpod_session(&contract_addr, 0)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        return;
    } else if cmd == "get-superblock" {
        if argv.len() < 3 {
            eprintln!(
                "Usage: {} wrbpod {} [-w wrbpod_contract_addr]",
                &argv[0], &cmd
            );
            process::exit(1);
        }
        let contract_addr = get_wrbpod_contract_id(&mut argv)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        setup_wrbpod_session(&contract_addr, 0)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        let superblock = with_globals(|globals| {
            let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
            let superblock = wrbpod_session.superblock().clone();
            superblock
        });

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
                "Usage: {} wrbpod {} [-w wrbpod_contract_addr] [-s app_source] APP_NAME NUM_SLOTS",
                &argv[0], &cmd
            );
            process::exit(1);
        }
        let contract_addr = get_wrbpod_contract_id(&mut argv)
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

        let bytes = load_wrbsite_source(&app_name, wrbsite_data_source_opt)
            .map_err(|e| {
                usage(&e);
                unreachable!()
            })
            .unwrap();

        let code_hash = Hash160::from_data(&bytes);

        setup_wrbpod_session(&contract_addr, 0)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        let res = with_globals(|globals| {
            let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
            wrbpod_session
                .allocate_slots(&app_name, code_hash, num_slots)
                .map_err(|e| {
                    eprintln!("FATAL: failed to allocate slots: {:?}", &e);
                    process::exit(1);
                })
                .unwrap()
        });

        if res {
            return;
        } else {
            eprintln!("Failed to allocate new slots");
            process::exit(1);
        }
    } else if cmd == "put-slot" {
        if argv.len() < 6 {
            eprintln!("Usage: {} wrbpod {} [-w wrbpod_contract_addr] APP_NAME APP_SLOT_NUM APP_SLOT_JSON_OR_STDIN", &argv[0], &cmd);
            process::exit(1);
        }
        let contract_addr = get_wrbpod_contract_id(&mut argv)
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

        setup_wrbpod_session(&contract_addr, 0)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        let Some(stackerdb_chunk_id) = with_globals(|globals| {
            let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
            wrbpod_session.app_slot_id_to_stackerdb_chunk_id(&app_name, app_slot_num)
        }) else {
            eprintln!(
                "FATAL: app slot {} is not occupied by app '{}'",
                app_slot_num, app_name
            );
            process::exit(1);
        };

        let slots_metadata = with_globals(|globals| {
            let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
            wrbpod_session.list_chunks()
        })
        .map_err(|e| {
            eprintln!(
                "FATAL: failed to list chunks in {}: {:?}",
                &contract_addr, &e
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
                    stackerdb_chunk_id, app_slot_num, &contract_addr
                );
                process::exit(1);
            });

        let chunk_data = slot.to_stackerdb_chunk(stackerdb_chunk_id, slot_version + 1);

        with_globals(|globals| {
            let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
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

        return;
    } else if cmd == "clear-slot" {
        if argv.len() < 6 {
            eprintln!(
                "Usage: {} wrbpod {} [-w wrbpod_contract_addr] APP_NAME APP_SLOT_NUM",
                &argv[0], &cmd
            );
            process::exit(1);
        }
        let contract_addr = get_wrbpod_contract_id(&mut argv)
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
        let slot = WrbpodSlices::new();

        setup_wrbpod_session(&contract_addr, 0)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        let Some(stackerdb_chunk_id) = with_globals(|globals| {
            let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
            wrbpod_session.app_slot_id_to_stackerdb_chunk_id(&app_name, app_slot_num)
        }) else {
            eprintln!(
                "FATAL: app slot {} is not occupied by app '{}'",
                app_slot_num, app_name
            );
            process::exit(1);
        };

        let slots_metadata = with_globals(|globals| {
            let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
            wrbpod_session.list_chunks()
        })
        .map_err(|e| {
            eprintln!(
                "FATAL: failed to list chunks in {}: {:?}",
                &contract_addr, &e
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
                    stackerdb_chunk_id, app_slot_num, &contract_addr
                );
                process::exit(1);
            });

        let chunk_data = slot.to_stackerdb_chunk(stackerdb_chunk_id, slot_version + 1);

        with_globals(|globals| {
            let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
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

        return;
    } else if cmd == "get-chunk" {
        if argv.len() < 4 {
            eprintln!(
                "Usage: {} wrbpod {} [-w wrbpod_contract_addr] SLOT_NUM",
                &argv[0], &cmd
            );
            process::exit(1);
        }
        let contract_addr = get_wrbpod_contract_id(&mut argv)
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

        setup_wrbpod_session(&contract_addr, 0)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        let slot = with_globals(|globals| {
            let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
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
                "Usage: {} wrbpod get-slot [-w wrbpod_contract_addr] APP_NAME APP_SLOT_NUM",
                &argv[0]
            );
            process::exit(1);
        }
        let contract_addr = get_wrbpod_contract_id(&mut argv)
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

        let slot = get_wrbpod_slot(&contract_addr, &app_name, app_slot_num)
            .map_err(|e| {
                eprintln!(
                    "FATAL: failed to get wrbpod slot {} for app '{}' in contract '{}': {:?}",
                    app_slot_num, &app_name, &contract_addr, &e
                );
                process::exit(1);
            })
            .unwrap();

        if let Some(slot) = slot {
            println!("{}", &serde_json::to_string(&slot).unwrap());
        } else {
            eprintln!("No such slot");
        }
        return;
    } else if cmd == "get-slice" {
        if argv.len() < 7 {
            eprintln!(
                "Usage: {} wrbpod {} [-w wrbpod_contract_addr] APP_NAME APP_SLOT_NUM APP_SLICE_ID",
                &argv[0], &cmd
            );
            process::exit(1);
        }
        let contract_addr = get_wrbpod_contract_id(&mut argv)
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

        let slot = get_wrbpod_slot(&contract_addr, &app_name, app_slot_num)
            .map_err(|e| {
                eprintln!(
                    "FATAL: failed to get wrbpod slot {} for app '{}' in contract '{}': {:?}",
                    app_slot_num, &app_name, &contract_addr, &e
                );
                process::exit(1);
            })
            .unwrap();

        if let Some(slot) = slot {
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
            eprintln!("Usage: {} wrbpod {} [-w wrbpod_contract_addr] APP_NAME APP_SLOT_NUM APP_SLICE_ID APP_SLICE_DATA", &argv[0], &cmd);
            process::exit(1);
        }
        let contract_addr = get_wrbpod_contract_id(&mut argv)
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

        setup_wrbpod_session(&contract_addr, 0)
            .map_err(|e| {
                eprintln!("FATAL: {}", &e);
                process::exit(1);
            })
            .unwrap();

        with_globals(|globals| {
            let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
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
        return;
    }
    eprintln!("Unrecognized `wrbpod` command '{}'", &cmd);
    process::exit(1);
}

fn main() {
    let mut argv: Vec<String> = env::args().collect();

    // get config with -c/--config
    let conf_path = if let Some(conf_path) = consume_arg(&mut argv, &["-c", "--config"], true)
        .map_err(|e| {
            usage(&e);
            unreachable!()
        })
        .unwrap()
    {
        conf_path
    } else {
        let home_dir = dirs::home_dir().unwrap_or(".".into());
        let wrb_conf = format!("{}/{}", &home_dir.display(), &DEFAULT_CONFIG);
        wrb_conf
    };

    // get debug log with -d/--debug-log
    let debug_path_opt = consume_arg(&mut argv, &["-d", "--debug-log"], true)
        .map_err(|e| {
            usage(&e);
            unreachable!()
        })
        .unwrap();

    // get the wrbsite data source, if given
    let wrbsite_data_source_opt = consume_arg(&mut argv, &["-s", "--source"], true)
        .map_err(|e| {
            usage(&e);
            unreachable!()
        })
        .unwrap();

    // get the wrb page ID or command name
    if argv.len() < 2 {
        usage("Expected a wrbsite");
        unreachable!()
    }

    let wrbsite_name = argv[1].clone();

    // create the config file if it doesn't exist
    if fs::metadata(&conf_path).is_err() {
        let default_conf = Config::default(true, "localhost", 20443);
        let default_conf_str = toml::to_string(&default_conf).unwrap();

        let conf_pathbuf = Path::new(&conf_path);
        if let Some(conf_dir) = conf_pathbuf.parent() {
            fs::create_dir_all(&conf_dir).unwrap();
        }
        fs::write(&conf_path, default_conf_str)
            .map_err(|e| {
                eprintln!(
                    "FATAL: failed to write default config to '{}': {:?}",
                    &conf_path, &e
                );
                process::exit(1);
            })
            .unwrap()
    }

    // load up config
    let conf = Config::from_path(&conf_path)
        .map_err(|e| {
            usage(&format!(
                "Could not load config from '{}': {}",
                &conf_path, &e
            ));
            unreachable!()
        })
        .unwrap();

    // set up the wrb client
    let db_path = conf.db_path();
    if fs::metadata(&db_path).is_err() {
        fs::create_dir_all(&db_path)
            .map_err(|e| {
                eprintln!("FATAL: failed to create directory '{}': {:?}", &db_path, &e);
                process::exit(1);
            })
            .unwrap();
    }
    core::init_config(conf.clone());

    // this might be a command instead of a wrbsite
    let cmd = argv[1].clone();
    if cmd == "clarity" {
        // clarity tooling mode
        subcommand_clarity(argv);
        process::exit(0);
    } else if cmd == "wrbpod" {
        // wrbpod tooling mode
        subcommand_wrbpod(argv, wrbsite_data_source_opt);
        process::exit(0);
    }

    redirect_logfile(&debug_path_opt.unwrap_or(conf.debug_path())).unwrap();

    wrb_debug!("Booted up");

    let bytes = load_wrbsite_source(&wrbsite_name, wrbsite_data_source_opt)
        .map_err(|e| {
            usage(&e);
            unreachable!()
        })
        .unwrap();

    // load the page
    let mut vm = ClarityVM::new(&db_path, &wrbsite_name).expect("Failed to instantiate ClarityVM");
    let renderer = Renderer::new(1_000_000_000);

    let (render_channels, ui_channels) = WrbChannels::new();

    let event_pipe = ui_channels.get_event_sender();
    let viewer = Viewer::new(ui_channels, &wrbsite_name);

    let render_handle = thread::spawn(move || renderer.run_page(&mut vm, &bytes, render_channels));

    let _ = viewer.main();
    let _ = event_pipe.send(WrbEvent::Close);
    let _ = render_handle.join();
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::fs::File;
    use std::io::Write;

    use clarity::vm::types::QualifiedContractIdentifier;
    use clarity::vm::Value;

    use crate::ui::ValueExtensions;

    use crate::core;
    use crate::core::with_globals;
    use crate::json_to_clarity;
    use crate::setup_wrbpod_session;

    use crate::subcommand_wrbpod;

    use crate::stacks_common::codec::StacksMessageCodec;
    use stacks_common::util::hash::to_hex;

    use crate::storage::WrbpodSlices;
    use crate::storage::WrbpodSuperblock;

    #[test]
    fn test_json_to_clarity() {
        let json_str = r#"{ "a": 1, "b": "\"hello world\"", "c": { "d": false, "e": null, "f": [ "u\"ghij\"", "u\"klm\"", "u\"n\"" ] }, "g": "u1", "h": "(+ 1 2)", "i": -1}"#;
        let val = json_to_clarity(&mut json_str.as_bytes()).unwrap();

        eprintln!("{:?}", &val);

        let val_tuple = val.expect_tuple().unwrap();
        let val_a = val_tuple.get("a").cloned().unwrap().expect_i128().unwrap();
        assert_eq!(val_a, 1);

        let val_b = val_tuple.get("b").cloned().unwrap().expect_ascii().unwrap();
        assert_eq!(val_b, "hello world");

        let val_c_tuple = val_tuple.get("c").cloned().unwrap().expect_tuple().unwrap();
        let val_d = val_c_tuple
            .get("d")
            .cloned()
            .unwrap()
            .expect_bool()
            .unwrap();
        assert_eq!(val_d, false);

        let val_e = val_c_tuple
            .get("e")
            .cloned()
            .unwrap()
            .expect_optional()
            .unwrap();
        assert_eq!(val_e, None);

        let val_f = val_c_tuple
            .get("f")
            .cloned()
            .unwrap()
            .expect_list()
            .unwrap();
        for (i, val) in val_f.into_iter().enumerate() {
            let val_str = val.expect_utf8().unwrap();

            if i == 0 {
                assert_eq!(val_str, "ghij");
            }
            if i == 1 {
                assert_eq!(val_str, "klm");
            }
            if i == 2 {
                assert_eq!(val_str, "n");
            }
        }

        let val_g = val_tuple.get("g").cloned().unwrap().expect_u128().unwrap();
        assert_eq!(val_g, 1);

        let val_h = val_tuple.get("h").cloned().unwrap().expect_i128().unwrap();
        assert_eq!(val_h, 3);

        let val_i = val_tuple.get("i").cloned().unwrap().expect_i128().unwrap();
        assert_eq!(val_i, -1);

        // this will fail due to incompatible list types
        let json_str = r#"[ 1, false, "abc"]"#;
        assert!(json_to_clarity(&mut json_str.as_bytes()).is_err());
    }

    #[test]
    fn test_wrbpod_format() {
        core::init(true, "localhost", 20443);

        let wrb_src_path = "/tmp/test-wrbpod-format.clar";
        if fs::metadata(&wrb_src_path).is_ok() {
            fs::remove_file(&wrb_src_path).unwrap();
        }
        let mut wrb_src = fs::File::create(&wrb_src_path).unwrap();
        wrb_src.write_all(br#"(print "hello world")"#).unwrap();
        drop(wrb_src);

        let contract_addr =
            QualifiedContractIdentifier::parse("SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod")
                .unwrap();
        let args = vec![
            "wrb-test".to_string(),
            "wrbpod".to_string(),
            "format".to_string(),
            "-w".to_string(),
            contract_addr.to_string(),
            "hello-formats.btc".to_string(),
            "1".to_string(),
        ];

        subcommand_wrbpod(args, Some(wrb_src_path.to_string()));

        // check superblock
        with_globals(|globals| {
            let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
            let superblock = wrbpod_session.superblock();
            assert_eq!(superblock, &WrbpodSuperblock::new());
        });
    }

    #[test]
    fn test_get_wrbpod_superblock() {
        core::init(true, "localhost", 20443);
        let contract_addr =
            QualifiedContractIdentifier::parse("SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod")
                .unwrap();
        setup_wrbpod_session(&contract_addr, 0).unwrap();

        let superblock = with_globals(|globals| {
            let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
            wrbpod_session.superblock().clone()
        });

        println!("{}", serde_json::to_string(&superblock).unwrap());
    }

    #[test]
    fn test_wrbpod_alloc_slots() {
        core::init(true, "localhost", 20443);

        let wrb_src_path = "/tmp/test-wrbpod-alloc-slots.clar";
        if fs::metadata(&wrb_src_path).is_ok() {
            fs::remove_file(&wrb_src_path).unwrap();
        }
        let mut wrb_src = fs::File::create(&wrb_src_path).unwrap();
        wrb_src.write_all(br#"(print "hello world")"#).unwrap();
        drop(wrb_src);

        let contract_addr =
            QualifiedContractIdentifier::parse("SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod")
                .unwrap();
        let args = vec![
            "wrb-test".to_string(),
            "wrbpod".to_string(),
            "alloc-slots".to_string(),
            "-w".to_string(),
            contract_addr.to_string(),
            "hello-alloc-slots.btc".to_string(),
            "1".to_string(),
        ];

        subcommand_wrbpod(args, Some(wrb_src_path.to_string()));

        // check superblock
        let app_state = with_globals(|globals| {
            let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
            let superblock = wrbpod_session.superblock();
            let app_state = superblock.app_state("hello-alloc-slots.btc").unwrap();
            (*app_state).clone()
        });

        assert_eq!(app_state.slots, vec![1]);
    }

    #[test]
    fn test_wrbpod_get_put_slot() {
        core::init(true, "localhost", 20443);

        let wrb_src_path = "/tmp/test-wrbpod-get-put-slot.clar";
        if fs::metadata(&wrb_src_path).is_ok() {
            fs::remove_file(&wrb_src_path).unwrap();
        }
        let mut wrb_src = fs::File::create(&wrb_src_path).unwrap();
        wrb_src.write_all(br#"(print "hello world")"#).unwrap();
        drop(wrb_src);

        // need to alloc slots first
        let contract_addr =
            QualifiedContractIdentifier::parse("SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod")
                .unwrap();
        let args = vec![
            "wrb-test".to_string(),
            "wrbpod".to_string(),
            "alloc-slots".to_string(),
            "-w".to_string(),
            contract_addr.to_string(),
            "hello-get-put-slot.btc".to_string(),
            "1".to_string(),
        ];

        subcommand_wrbpod(args, Some(wrb_src_path.to_string()));

        // check superblock
        let app_state = with_globals(|globals| {
            let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
            let superblock = wrbpod_session.superblock();
            let app_state = superblock.app_state("hello-get-put-slot.btc").unwrap();
            (*app_state).clone()
        });

        assert_eq!(app_state.slots, vec![1]);

        // make a slice
        let mut slices = WrbpodSlices::new();
        slices.put_slice(0, vec![1, 2, 3, 4, 5]);

        // put the slot
        let args = vec![
            "wrb-test".to_string(),
            "wrbpod".to_string(),
            "put-slot".to_string(),
            "-w".to_string(),
            contract_addr.to_string(),
            "hello-get-put-slot.btc".to_string(),
            "0".to_string(),
            serde_json::to_string(&slices).unwrap(),
        ];

        subcommand_wrbpod(args, Some(wrb_src_path.to_string()));

        // go get the slot
        let mut slot = with_globals(|globals| {
            let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
            wrbpod_session
                .fetch_chunk("hello-get-put-slot.btc", 0)
                .map_err(|e| {
                    panic!("FATAL: failed to fetch chunk: {:?}", &e);
                })
                .unwrap();

            let chunk_ref = wrbpod_session.ref_chunk(1).unwrap();

            (*chunk_ref).clone()
        });

        slot.set_dirty(true);
        assert_eq!(slot, slices);
    }
}
