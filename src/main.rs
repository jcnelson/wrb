#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

#[macro_use]
extern crate stacks_common;

#[macro_use]
extern crate serde_json;

#[macro_use]
extern crate clarity;

#[macro_use]
extern crate lazy_static;

extern crate libstackerdb;

extern crate base64ct;
extern crate dirs;
extern crate lzma_rs;
extern crate regex;
extern crate rusqlite;
extern crate termion;

#[macro_use]
pub mod util;

pub mod cli;
pub mod core;
pub mod net;
pub mod runner;
pub mod storage;
pub mod tx;
pub mod ui;
pub mod viewer;
pub mod vm;

use std::env;
use std::fs;
use std::io::{stdin, stdout, Read};
use std::path::Path;
use std::process;
use std::thread;
use std::time::Duration;

use crate::core::Config;
use crate::core::ConfigFile;
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

use cli::{
    consume_arg, load_wrbsite_source, make_runner, split_fqn, subcommand_bns, subcommand_clarity,
    subcommand_site, subcommand_wrbpod, usage,
};

const DEFAULT_CONFIG: &str = ".wrb/config.toml";

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
        let default_conf_file = ConfigFile::from(default_conf);
        let default_conf_str = toml::to_string(&default_conf_file).unwrap();

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
    } else if cmd == "bns" {
        // bns tooling mode
        subcommand_bns(argv);
        process::exit(0);
    } else if cmd == "site" {
        // site tooling mode
        subcommand_site(argv);
        process::exit(0);
    }

    redirect_logfile(&debug_path_opt.unwrap_or(conf.debug_path())).unwrap();

    wrb_debug!("Booted up");

    let (bytes, version) = load_wrbsite_source(&wrbsite_name, wrbsite_data_source_opt)
        .map_err(|e| {
            usage(&e);
            unreachable!()
        })
        .unwrap();

    // load the page
    let mut vm =
        ClarityVM::new(&db_path, &wrbsite_name, version).expect("Failed to instantiate ClarityVM");
    let mut renderer = Renderer::new(1_000_000_000);

    let (render_channels, ui_channels) = WrbChannels::new();

    let event_pipe = ui_channels.get_event_sender();
    let viewer = Viewer::new(ui_channels, &wrbsite_name);

    let render_event_pipe = event_pipe.clone();
    let render_handle = thread::spawn(move || {
        if let Err(e) = renderer.run_page(&mut vm, &bytes, render_channels) {
            wrb_error!("Failed to run page: {:?}", &e);
            let _ = render_event_pipe.send(WrbEvent::Close);
        }
    });

    let _ = viewer.main();
    let _ = event_pipe.send(WrbEvent::Close);
    let _ = render_handle.join();
    process::exit(0);
}
