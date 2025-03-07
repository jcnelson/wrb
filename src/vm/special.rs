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

use crate::vm::storage::WrbDB;
use crate::vm::storage::WrbHeadersDB;
use std::ops::Deref;
use std::str;

use crate::runner::Runner;

use crate::storage::Wrbpod;

use crate::core::make_runner;
use crate::core::with_global_config;
use crate::core::with_globals;

use crate::util::privkey_to_principal;

use crate::storage::Error as WrbpodError;
use crate::storage::WrbpodAddress;

use crate::vm::WRB_CONTRACT;
use crate::vm::WRB_LOW_LEVEL_CONTRACT;

use clarity::boot_util::boot_code_addr;
use clarity::boot_util::boot_code_id;
use clarity::vm::ast::ASTRules;
use clarity::vm::contexts::{CallStack, Environment, EventBatch, GlobalContext};
use clarity::vm::contracts::Contract;
use clarity::vm::errors::{Error, InterpreterError};
use clarity::vm::representations::{ClarityName, SymbolicExpression, SymbolicExpressionType};
use clarity::vm::types::{
    ASCIIData, BuffData, OptionalData, PrincipalData, QualifiedContractIdentifier, ResponseData,
    SequenceData, StandardPrincipalData, TupleData, TypeSignature, Value,
};
use clarity::vm::ClarityVersion;
use clarity::vm::ContractContext;

use stacks_common::types::chainstate::StacksPrivateKey;
use stacks_common::util::hash::{to_hex, Hash160};

use crate::runner::stackerdb::StackerDBSession;

pub const WRB_ERROR_INFALLIBLE: u128 = 0;
pub const WRB_ERR_INVALID: u128 = 1;

pub const WRB_ERR_WRBPOD_NOT_OPEN: u128 = 1000;
pub const WRB_ERR_WRBPOD_NO_SLOT: u128 = 1001;
pub const WRB_ERR_WRBPOD_NO_SLICE: u128 = 1002;
pub const WRB_ERR_WRBPOD_OPEN_FAILURE: u128 = 1003;
pub const WRB_ERR_WRBPOD_SLOT_ALLOC_FAILURE: u128 = 1004;
pub const WRB_ERR_WRBPOD_FETCH_SLOT_FAILURE: u128 = 1005;
pub const WRB_ERR_WRBPOD_PUT_SLICE_FAILURE: u128 = 1006;
pub const WRB_ERR_WRBPOD_SYNC_SLOT_FAILURE: u128 = 1007;

pub const WRB_ERR_READONLY_FAILURE: u128 = 2000;

pub const WRB_ERR_BUFF_TO_UTF8_FAILURE: u128 = 3000;

pub const WRB_ERR_STRING_ASCII_TO_STRING_UTF8_FAILURE: u128 = 4000;

fn env_with_global_context<F, A, E>(
    global_context: &mut GlobalContext,
    sender: PrincipalData,
    sponsor: Option<PrincipalData>,
    contract_context: ContractContext,
    f: F,
) -> std::result::Result<A, E>
where
    E: From<clarity::vm::errors::Error>,
    F: FnOnce(&mut Environment) -> std::result::Result<A, E>,
{
    global_context.begin();

    let result = {
        let mut callstack = CallStack::new();
        let mut exec_env = Environment::new(
            global_context,
            &contract_context,
            &mut callstack,
            Some(sender.clone()),
            Some(sender),
            sponsor,
        );
        f(&mut exec_env)
    };
    let _ = global_context.commit()?;
    result
}

/// Make an (err { code: uint, message: (string-ascii 512) })
fn err_ascii_512(code: u128, msg: &str) -> Value {
    Value::error(Value::Tuple(
        TupleData::from_data(vec![
            ("code".into(), Value::UInt(code)),
            (
                "message".into(),
                Value::string_ascii_from_bytes(msg.as_bytes().to_vec())
                    .expect("FATAL: failed to construct value from string-ascii"),
            ),
        ])
        .expect("FATAL: failed to build valid tuple"),
    ))
    .expect("FATAL: failed to construct error tuple")
}

/// Trampoline code for contract-call to `.wrb call-readonly`
fn handle_wrb_call_readonly(
    global_context: &mut GlobalContext,
    sender: PrincipalData,
    sponsor: Option<PrincipalData>,
    contract_id: &QualifiedContractIdentifier,
    args: &[Value],
    wrb_lowlevel_contract: Contract,
    mut runner: Runner,
) -> Result<(), Error> {
    // must be 3 arguments -- contract ID, function name, and the serialized list
    if args.len() != 3 {
        return Err(InterpreterError::InterpreterError(format!(
            "Expected 3 arguments, got {}",
            args.len()
        ))
        .into());
    }

    let contract_id_value = args[0].clone().expect_principal()?;
    let function_name = args[1].clone().expect_ascii()?;
    let args_buff = to_hex(&args[2].clone().expect_buff(102400)?);
    let args_list_value = Value::try_deserialize_hex_untyped(&args_buff).map_err(|e| {
        InterpreterError::InterpreterError(format!("Failed to decode args list: {:?}", &e))
    })?;

    let args_list = args_list_value.expect_list()?;
    let mut args = vec![];
    for (i, arg) in args_list.into_iter().enumerate() {
        let Value::Sequence(SequenceData::Buffer(buff_data)) = arg else {
            return Err(InterpreterError::InterpreterError(format!(
                "Value argument {} is not a serialized value",
                i
            ))
            .into());
        };
        let val_hex = to_hex(&buff_data.data);
        let val = Value::try_deserialize_hex_untyped(&val_hex).map_err(|e| {
            InterpreterError::InterpreterError(
                format!("Failed to decode argument {}: {:?}", i, &e).into(),
            )
        })?;

        wrb_debug!("arg: {:?}", &val);
        args.push(val);
    }

    let PrincipalData::Contract(target_contract_id) = contract_id_value else {
        return Err(
            InterpreterError::InterpreterError("Expected contract principal".into()).into(),
        );
    };

    // carry out the RPC
    let value = match runner.call_readonly(&target_contract_id, &function_name, &args) {
        Ok(value) => Value::okay(Value::buff_from(value.serialize_to_vec()?).unwrap()).unwrap(),
        Err(e) => err_ascii_512(
            WRB_ERR_READONLY_FAILURE,
            &format!("wrb: failed call-readonly: {:?}", &e),
        ),
    };

    env_with_global_context(
        global_context,
        sender,
        sponsor,
        wrb_lowlevel_contract.contract_context,
        |env| {
            env.execute_contract_allow_private(
                &contract_id,
                "set-last-call-readonly",
                &[SymbolicExpression::atom_value(value)],
                false,
            )
        },
    )
    .expect("FATAL: failed to set read-only call result");
    Ok(())
}

/// Trampoline code for contract-call to `.wrb buff-to-string-utf8`
fn handle_buff_to_string_utf8(
    global_context: &mut GlobalContext,
    sender: PrincipalData,
    sponsor: Option<PrincipalData>,
    contract_id: &QualifiedContractIdentifier,
    args: &[Value],
    wrb_lowlevel_contract: Contract,
) -> Result<(), Error> {
    // must be one argument
    if args.len() != 1 {
        return Err(InterpreterError::InterpreterError(format!(
            "Expected 1 argument, got {}",
            args.len()
        ))
        .into());
    }

    let hex_buff = args[0].clone().expect_buff(102400)?;
    let value = match std::str::from_utf8(&hex_buff) {
        Ok(s) => Value::okay(Value::string_utf8_from_string_utf8_literal(s.to_string()).unwrap())
            .unwrap(),
        Err(e) => err_ascii_512(
            WRB_ERR_BUFF_TO_UTF8_FAILURE,
            &format!("wrb: failed to decode buffer to UTF-8: {:?}", &e),
        ),
    };

    env_with_global_context(
        global_context,
        sender,
        sponsor,
        wrb_lowlevel_contract.contract_context,
        |env| {
            env.execute_contract_allow_private(
                contract_id,
                "set-last-wrb-buff-to-string-utf8",
                &[SymbolicExpression::atom_value(value)],
                false,
            )
        },
    )
    .expect("FATAL: failed to set last wrb-to-utf8 request");
    Ok(())
}

/// Trampoline code for contract-call to `.wrb string-ascii-to-string-utf8`
fn handle_string_ascii_to_string_utf8(
    global_context: &mut GlobalContext,
    sender: PrincipalData,
    sponsor: Option<PrincipalData>,
    contract_id: &QualifiedContractIdentifier,
    args: &[Value],
    wrb_lowlevel_contract: Contract,
) -> Result<(), Error> {
    // must be one argument
    if args.len() != 1 {
        return Err(InterpreterError::InterpreterError(format!(
            "Expected 1 argument, got {}",
            args.len()
        ))
        .into());
    }

    let ascii_str = args[0].clone().expect_ascii()?;
    let value = if ascii_str.len() < 25600 {
        Value::okay(Value::string_utf8_from_string_utf8_literal(ascii_str).unwrap()).unwrap()
    } else {
        err_ascii_512(
            WRB_ERR_STRING_ASCII_TO_STRING_UTF8_FAILURE,
            "wrb: failed to convert ASCII to UTF-8: too big",
        )
    };

    env_with_global_context(
        global_context,
        sender,
        sponsor,
        wrb_lowlevel_contract.contract_context,
        |env| {
            env.execute_contract_allow_private(
                contract_id,
                "set-last-wrb-string-ascii-to-string-utf8",
                &[SymbolicExpression::atom_value(value)],
                false,
            )
        },
    )
    .expect("FATAL: failed to set last wrb-to-utf8 request");
    Ok(())
}

/// Trampoline code for contract-call to `.wrb wrbpod-default`
pub fn handle_wrbpod_default(
    global_context: &mut GlobalContext,
    sender: PrincipalData,
    sponsor: Option<PrincipalData>,
    contract_id: &QualifiedContractIdentifier,
    args: &[Value],
    wrb_lowlevel_contract: Contract,
) -> Result<(), Error> {
    // no args
    if args.len() != 0 {
        return Err(InterpreterError::InterpreterError(format!(
            "Expected 0 arguments, got {}",
            args.len()
        ))
        .into());
    }

    let default_superblock =
        with_global_config(|cfg| cfg.default_wrbpod().clone()).expect("System is not initialized");

    let default_superblock_value = Value::Tuple(
        TupleData::from_data(vec![
            (
                "contract".into(),
                Value::Principal(default_superblock.contract.clone().into()),
            ),
            ("slot".into(), Value::UInt(default_superblock.slot.into())),
        ])
        .unwrap(),
    );

    env_with_global_context(
        global_context,
        sender,
        sponsor,
        wrb_lowlevel_contract.contract_context,
        |env| {
            env.execute_contract_allow_private(
                contract_id,
                "finish-wrbpod-default",
                &[SymbolicExpression::atom_value(default_superblock_value)],
                false,
            )
        },
    )
    .expect("FATAL: failed to set default wrbpod");
    Ok(())
}

/// Trampoline code for contract-call to `.wrb wrbpod-open`
pub fn handle_wrbpod_open(
    global_context: &mut GlobalContext,
    sender: PrincipalData,
    sponsor: Option<PrincipalData>,
    contract_id: &QualifiedContractIdentifier,
    args: &[Value],
    wrb_lowlevel_contract: Contract,
) -> Result<(), Error> {
    // must be one argument
    if args.len() != 1 {
        return Err(InterpreterError::InterpreterError(format!(
            "Expected 1 argument, got {}",
            args.len()
        ))
        .into());
    }

    let superblock_tuple = args[0].clone().expect_tuple()?;
    let PrincipalData::Contract(wrbpod_contract_id) = superblock_tuple
        .get("contract")
        .expect("FATAL: missing `contract` in wrbpod superblock")
        .clone()
        .expect_principal()?
    else {
        return Err(InterpreterError::InterpreterError(format!(
            "Expected a contract principal for wrbpod-open superblock tuple",
        ))
        .into());
    };

    let wrbpod_slot_id = superblock_tuple
        .get("slot")
        .expect("FATAL: missing `slot` in wrbpod superblock")
        .clone()
        .expect_u128()?;

    let wrbpod_slot_id: u32 = match wrbpod_slot_id.try_into() {
        Ok(x) => x,
        Err(_) => {
            wrb_warn!("Invalid wrbpod superblock slot ID {}", wrbpod_slot_id);
            let result = err_ascii_512(
                WRB_ERR_WRBPOD_OPEN_FAILURE,
                "wrb: invalid superblock slot ID. Must be 32-bit.".into(),
            );
            env_with_global_context(
                global_context,
                sender,
                sponsor,
                wrb_lowlevel_contract.contract_context,
                |env| {
                    env.execute_contract_allow_private(
                        contract_id,
                        "finish-wrbpod-open",
                        &[
                            SymbolicExpression::atom_value(args[0].clone()),
                            SymbolicExpression::atom_value(Value::UInt(0)),
                            SymbolicExpression::atom_value(result),
                        ],
                        false,
                    )
                },
            )?;
            return Ok(());
        }
    };

    let wrbpod_addr = WrbpodAddress {
        contract: wrbpod_contract_id.clone(),
        slot: wrbpod_slot_id,
    };

    // is this an owned wrbpod? only true if the client's identity private key matches the target
    // contract.
    let privkey = with_global_config(|cfg| cfg.private_key().clone()).ok_or(
        InterpreterError::InterpreterError(format!("System is not initialized")),
    )?;

    let key_principal = privkey_to_principal(&privkey, wrbpod_contract_id.issuer.version());
    let owned = key_principal == wrbpod_contract_id.issuer;

    wrb_debug!(
        "Will open wrbpod {} (owned? {})",
        &wrbpod_contract_id,
        owned
    );

    let opened_session_id =
        with_globals(|globals| globals.get_wrbpod_session_id_by_address(&wrbpod_addr));
    if let Some(wrbpod_session_id) = opened_session_id {
        // this wrbpod is already open, so complete the opening
        wrb_debug!(
            "Wrbpod {} already open: session ID {}",
            &wrbpod_contract_id,
            wrbpod_session_id
        );
        let result = Value::okay(Value::Bool(owned)).unwrap();
        env_with_global_context(
            global_context,
            sender,
            sponsor,
            wrb_lowlevel_contract.contract_context,
            |env| {
                env.execute_contract_allow_private(
                    contract_id,
                    "finish-wrbpod-open",
                    &[
                        SymbolicExpression::atom_value(args[0].clone()),
                        SymbolicExpression::atom_value(Value::UInt(wrbpod_session_id)),
                        SymbolicExpression::atom_value(result),
                    ],
                    false,
                )
            },
        )?;
        return Ok(());
    }

    let mut runner = make_runner();

    // go set up the wrbpod session
    let home_stackerdb_client =
        match runner.get_home_stackerdb_client(wrbpod_contract_id.clone(), privkey.clone()) {
            Ok(client) => client,
            Err(e) => {
                wrb_warn!(
                    "Failed to open home StackerDB client for {}: {:?}",
                    &wrbpod_contract_id,
                    &e
                );
                let result = err_ascii_512(
                    WRB_ERR_WRBPOD_OPEN_FAILURE,
                    &format!(
                        "wrb: failed to open home StackerDB client for {}: {:?}",
                        &wrbpod_contract_id, &e
                    ),
                );
                env_with_global_context(
                    global_context,
                    sender,
                    sponsor,
                    wrb_lowlevel_contract.contract_context,
                    |env| {
                        env.execute_contract_allow_private(
                            contract_id,
                            "finish-wrbpod-open",
                            &[
                                SymbolicExpression::atom_value(args[0].clone()),
                                SymbolicExpression::atom_value(Value::UInt(0)),
                                SymbolicExpression::atom_value(result),
                            ],
                            false,
                        )
                    },
                )?;
                return Ok(());
            }
        };

    let replica_stackerdb_client =
        match runner.get_replica_stackerdb_client(wrbpod_contract_id.clone(), privkey.clone()) {
            Ok(client) => client,
            Err(e) => {
                wrb_warn!(
                    "Failed to open replica StackerDB client for {}: {:?}",
                    &wrbpod_contract_id,
                    &e
                );
                let result = err_ascii_512(
                    WRB_ERR_WRBPOD_OPEN_FAILURE,
                    &format!(
                        "wrb: failed to open replica StackerDB client for {}: {:?}",
                        &wrbpod_contract_id, &e
                    ),
                );
                env_with_global_context(
                    global_context,
                    sender,
                    sponsor,
                    wrb_lowlevel_contract.contract_context,
                    |env| {
                        env.execute_contract_allow_private(
                            contract_id,
                            "finish-wrbpod-open",
                            &[
                                SymbolicExpression::atom_value(args[0].clone()),
                                SymbolicExpression::atom_value(Value::UInt(0)),
                                SymbolicExpression::atom_value(result),
                            ],
                            false,
                        )
                    },
                )?;
                return Ok(());
            }
        };

    let wrbpod_session_result = Wrbpod::open(
        home_stackerdb_client,
        replica_stackerdb_client,
        privkey.clone(),
        wrbpod_slot_id,
    )
    .map_err(|e| {
        let msg = format!(
            "Failed to open wrbpod session to {}: {:?}",
            &wrbpod_contract_id, &e
        );
        wrb_warn!("{}", &msg);
        msg
    });

    match wrbpod_session_result {
        Ok(wrbpod_session) => {
            let result = Value::okay(Value::Bool(owned)).unwrap();
            let wrbpod_session_id = with_globals(|globals| globals.next_wrbpod_session_id());
            env_with_global_context(
                global_context,
                sender,
                sponsor,
                wrb_lowlevel_contract.contract_context,
                |env| {
                    env.execute_contract_allow_private(
                        contract_id,
                        "finish-wrbpod-open",
                        &[
                            SymbolicExpression::atom_value(args[0].clone()),
                            SymbolicExpression::atom_value(Value::UInt(wrbpod_session_id)),
                            SymbolicExpression::atom_value(result),
                        ],
                        false,
                    )
                },
            )?;

            with_globals(|globals| {
                globals.add_wrbpod_session(wrbpod_session_id, wrbpod_addr.clone(), wrbpod_session)
            });
            wrb_info!(
                "Opened wrbpod session {} on {}",
                wrbpod_session_id,
                &wrbpod_contract_id
            );
        }
        Err(e) => {
            let result = err_ascii_512(
                WRB_ERR_WRBPOD_OPEN_FAILURE,
                &format!(
                    "wrb: failed to open wrbpod session to {}: {:?}",
                    &wrbpod_contract_id, &e
                ),
            );
            wrb_warn!(
                "Failed to open wrbpod session to {}: {:?}",
                &wrbpod_contract_id,
                &e
            );
            env_with_global_context(
                global_context,
                sender,
                sponsor,
                wrb_lowlevel_contract.contract_context,
                |env| {
                    env.execute_contract_allow_private(
                        contract_id,
                        "finish-wrbpod-open",
                        &[
                            SymbolicExpression::atom_value(args[0].clone()),
                            SymbolicExpression::atom_value(Value::UInt(0)),
                            SymbolicExpression::atom_value(result),
                        ],
                        false,
                    )
                },
            )?;
        }
    }
    Ok(())
}

/// Trampoline code for contract-call to `.wrb wrbpod-get-num-slots`
pub fn handle_wrbpod_get_num_slots(
    global_context: &mut GlobalContext,
    sender: PrincipalData,
    sponsor: Option<PrincipalData>,
    contract_id: &QualifiedContractIdentifier,
    args: &[Value],
    wrb_lowlevel_contract: Contract,
) -> Result<(), Error> {
    // must be two arguments
    if args.len() != 2 {
        return Err(InterpreterError::InterpreterError(format!(
            "Expected 2 arguments, got {}",
            args.len()
        ))
        .into());
    }

    let session_id = args[0].clone().expect_u128()?;
    let app_name_tuple = args[1].clone().expect_tuple()?;
    let app_name_buff = app_name_tuple
        .get("name")
        .expect("FATAL: missing 'name'")
        .clone()
        .expect_buff(48)?;

    let app_name_str = str::from_utf8(&app_name_buff).map_err(|_e| {
        wrb_warn!("Failed to decode name buffer to string");
        Error::Interpreter(InterpreterError::InterpreterError(format!(
            "Unable to convert name {:?} to UTF-8",
            &app_name_buff
        )))
    })?;

    let app_namespace_buff = app_name_tuple
        .get("namespace")
        .expect("FATAL: missing 'namespace'")
        .clone()
        .expect_buff(20)?;

    let app_namespace_str = str::from_utf8(&app_namespace_buff).map_err(|_e| {
        wrb_warn!("Failed to decode namespace buffer to string");
        Error::Interpreter(InterpreterError::InterpreterError(format!(
            "Unable to convert namespace {:?} to UTF-8",
            &app_name_tuple
        )))
    })?;

    let app_name = format!("{}.{}", &app_name_str, &app_namespace_str);
    let num_slots_res = with_globals(|globals| {
        let Some(wrbpod) = globals.get_wrbpod_session(session_id) else {
            wrb_warn!("No such wrbpod session {}", session_id);
            return Err("no such wrbpod session".to_string());
        };
        Ok(wrbpod.get_num_slots(&app_name))
    });

    let result = match num_slots_res {
        Ok(num_slots) => Value::okay(Value::UInt(num_slots.into())).unwrap(),
        Err(msg) => {
            wrb_warn!(
                "Failed to query number of slots for '{}': {}",
                &app_name,
                &msg
            );
            err_ascii_512(WRB_ERR_WRBPOD_NOT_OPEN, &msg)
        }
    };

    env_with_global_context(
        global_context,
        sender,
        sponsor,
        wrb_lowlevel_contract.contract_context,
        |env| {
            env.execute_contract_allow_private(
                contract_id,
                "set-last-wrbpod-get-num-slots",
                &[SymbolicExpression::atom_value(result)],
                false,
            )
        },
    )
    .expect("FATAL: failed to set last wrbpod-get-num-slots request");
    Ok(())
}

/// decode the result to a call to `get-app-name`
/// omits the version
fn load_app_name(
    global_context: &mut GlobalContext,
    sender: PrincipalData,
    sponsor: Option<PrincipalData>,
    wrb_lowlevel_contract: &Contract,
) -> (String, String) {
    let name_value = env_with_global_context(
        global_context,
        sender,
        sponsor,
        wrb_lowlevel_contract.contract_context.clone(),
        |env| {
            env.eval_read_only_with_rules(
                &wrb_lowlevel_contract.contract_context.contract_identifier,
                "(get-app-name)",
                ASTRules::PrecheckSize,
            )
        },
    )
    .expect("FATAL: failed to run `get-app-name`");

    let name_data = name_value
        .expect_tuple()
        .expect("FATAL: `get-app-name` did not eval to a tuple");
    let name_buff = name_data
        .get("name")
        .cloned()
        .expect("FATAL: missing `name`")
        .expect_buff(48)
        .expect("FATAL: name tuple does not have a valid `name`");
    let namespace_buff = name_data
        .get("namespace")
        .cloned()
        .expect("FATAL: missing `namespace`")
        .expect_buff(20)
        .expect("FATAL: name tuple does not have a valid `namespace`");

    let name = std::str::from_utf8(&name_buff).expect("FATAL: invalid `name` bytes");
    let namespace = std::str::from_utf8(&namespace_buff).expect("FATAL: invalid `namespace` bytes");

    (name.to_string(), namespace.to_string())
}

/// decode the result to a call to `get-app-code-hash`
fn load_app_code_hash(
    global_context: &mut GlobalContext,
    sender: PrincipalData,
    sponsor: Option<PrincipalData>,
    wrb_lowlevel_contract: &Contract,
) -> Hash160 {
    let hash_value = env_with_global_context(
        global_context,
        sender,
        sponsor,
        wrb_lowlevel_contract.contract_context.clone(),
        |env| {
            env.eval_read_only_with_rules(
                &wrb_lowlevel_contract.contract_context.contract_identifier,
                "(get-app-code-hash)",
                ASTRules::PrecheckSize,
            )
        },
    )
    .expect("FATAL: failed to run `get-app-code-hash`");

    let hash_buff = hash_value
        .expect_buff(20)
        .expect("FATAL: `get-app-code-hash` did not eval to a hash");
    let mut hash_bytes = [0u8; 20];
    hash_bytes[0..20].copy_from_slice(&hash_buff[0..20]);

    Hash160(hash_bytes)
}

/// Trampoline code for contract-call to `.wrb wrbpod-alloc-slots`
pub fn handle_wrbpod_alloc_slots(
    global_context: &mut GlobalContext,
    sender: PrincipalData,
    sponsor: Option<PrincipalData>,
    contract_id: &QualifiedContractIdentifier,
    args: &[Value],
    wrb_lowlevel_contract: Contract,
) -> Result<(), Error> {
    // must be two arguments
    if args.len() != 2 {
        return Err(InterpreterError::InterpreterError(format!(
            "Expected 2 arguments, got {}",
            args.len()
        ))
        .into());
    }

    let session_id = args[0].clone().expect_u128()?;
    let Ok(num_slots) = u32::try_from(args[1].clone().expect_u128()?) else {
        env_with_global_context(
            global_context,
            sender,
            sponsor,
            wrb_lowlevel_contract.contract_context,
            |env| {
                env.execute_contract_allow_private(
                    contract_id,
                    "set-last-wrbpod-alloc-slots-result",
                    &[SymbolicExpression::atom_value(err_ascii_512(
                        WRB_ERR_INVALID,
                        "too many slots",
                    ))],
                    false,
                )
            },
        )
        .expect("FATAL: failed to set last wrbpod-alloc-slots request");
        return Ok(());
    };

    // load the app name and code hash
    let (name, namespace) = load_app_name(
        global_context,
        sender.clone(),
        sponsor.clone(),
        &wrb_lowlevel_contract,
    );
    let code_hash = load_app_code_hash(
        global_context,
        sender.clone(),
        sponsor.clone(),
        &wrb_lowlevel_contract,
    );

    // allocate the slots
    let alloc_res = with_globals(|globals| {
        let Some(wrbpod) = globals.get_wrbpod_session(session_id) else {
            return Err(format!("No such session {}", session_id));
        };
        wrbpod
            .allocate_slots(&format!("{}.{}", &name, &namespace), code_hash, num_slots)
            .map_err(|e| format!("{:?}", &e))
    });

    let alloc_res_value = match alloc_res {
        Ok(res) => Value::okay(Value::Bool(res)).unwrap(),
        Err(msg) => err_ascii_512(WRB_ERR_WRBPOD_SLOT_ALLOC_FAILURE, &msg),
    };

    env_with_global_context(
        global_context,
        sender,
        sponsor,
        wrb_lowlevel_contract.contract_context,
        |env| {
            env.execute_contract_allow_private(
                contract_id,
                "set-last-wrbpod-alloc-slots-result",
                &[SymbolicExpression::atom_value(alloc_res_value)],
                false,
            )
        },
    )
    .expect("FATAL: failed to set last wrbpod-alloc-slots request");
    Ok(())
}

/// Trampoline code for contract-call to `.wrb wrbpod-fetch-slot`
/// (define-public (wrbpod-fetch-slot (session-id uint) (slot-id uint))
/// returns (response { version: uint, signer: principal } (string-ascii 512))
pub fn handle_wrbpod_fetch_slot(
    global_context: &mut GlobalContext,
    sender: PrincipalData,
    sponsor: Option<PrincipalData>,
    contract_id: &QualifiedContractIdentifier,
    args: &[Value],
    wrb_lowlevel_contract: Contract,
) -> Result<(), Error> {
    // must be two arguments
    if args.len() != 2 {
        return Err(InterpreterError::InterpreterError(format!(
            "Expected 2 arguments, got {}",
            args.len()
        ))
        .into());
    }

    let session_id = args[0].clone().expect_u128()?;
    let Ok(app_slot_id) = u32::try_from(args[1].clone().expect_u128()?) else {
        wrb_warn!("app slot is too big");
        env_with_global_context(
            global_context,
            sender,
            sponsor,
            wrb_lowlevel_contract.contract_context,
            |env| {
                env.execute_contract_allow_private(
                    contract_id,
                    "set-last-wrbpod-fetch-slot-result",
                    &[
                        SymbolicExpression::atom_value(Value::UInt(session_id)),
                        SymbolicExpression::atom_value(Value::UInt(args[1].clone().expect_u128()?)),
                        SymbolicExpression::atom_value(err_ascii_512(
                            WRB_ERR_INVALID,
                            "app slot is too big".into(),
                        )),
                    ],
                    false,
                )
            },
        )
        .expect("FATAL: failed to set last wrbpod-alloc-slots request");
        return Ok(());
    };

    let (name, namespace) = load_app_name(
        global_context,
        sender.clone(),
        sponsor.clone(),
        &wrb_lowlevel_contract,
    );

    // go fetch that app state chunk
    let fetch_res = with_globals(|globals| {
        let Some(wrbpod) = globals.get_wrbpod_session(session_id) else {
            wrb_warn!(
                "wrbpod.fetch_chunk({}.{}, {}): no such session {}",
                &name,
                &namespace,
                app_slot_id,
                session_id,
            );
            return Err("no such session".to_string());
        };
        match wrbpod.fetch_chunk(&format!("{}.{}", &name, &namespace), app_slot_id) {
            Ok(res) => Ok((res.0, res.1.map(|pk| pk.to_bytes_compressed()))),
            Err(WrbpodError::NoSuchChunk) => Ok((0, None)), // chunk is not yet written,
            Err(e) => {
                wrb_warn!(
                    "wrbpod.fetch_chunk({}.{}, {}): {:?}",
                    &name,
                    &namespace,
                    app_slot_id,
                    &e
                );
                Err(format!("{:?}", &e))
            }
        }
    });

    let fetch_res_value = match fetch_res {
        Ok(res) => Value::okay(Value::Tuple(
            TupleData::from_data(vec![
                ("version".into(), Value::UInt(res.0.into())),
                (
                    "signer".into(),
                    res.1
                        .map(|pk_bytes| Value::some(Value::buff_from(pk_bytes).unwrap()).unwrap())
                        .unwrap_or(Value::none()),
                ),
            ])
            .unwrap(),
        ))
        .unwrap(),
        Err(msg) => err_ascii_512(WRB_ERR_WRBPOD_FETCH_SLOT_FAILURE, &msg),
    };

    env_with_global_context(
        global_context,
        sender,
        sponsor,
        wrb_lowlevel_contract.contract_context,
        |env| {
            env.execute_contract_allow_private(
                contract_id,
                "set-last-wrbpod-fetch-slot-result",
                &[
                    SymbolicExpression::atom_value(Value::UInt(session_id)),
                    SymbolicExpression::atom_value(Value::UInt(app_slot_id.into())),
                    SymbolicExpression::atom_value(fetch_res_value),
                ],
                false,
            )
        },
    )
    .expect("FATAL: failed to set last wrbpod-fetch-slot request");
    Ok(())
}

/// Trampoline code for contract-call to `.wrb wrbpod-get-slice`
/// (define-public (wrbpod-get-slice (session-id uint) (slot-id uint) (slice-id uint))
/// returns (response (buff 786000) (string-ascii 512))
pub fn handle_wrbpod_get_slice(
    global_context: &mut GlobalContext,
    sender: PrincipalData,
    sponsor: Option<PrincipalData>,
    contract_id: &QualifiedContractIdentifier,
    args: &[Value],
    wrb_lowlevel_contract: Contract,
) -> Result<(), Error> {
    // must be three arguments
    if args.len() != 3 {
        return Err(InterpreterError::InterpreterError(format!(
            "Expected 3 arguments, got {}",
            args.len()
        ))
        .into());
    }

    let session_id = args[0].clone().expect_u128()?;
    let slice_id = args[2].clone().expect_u128()?;
    let Ok(app_slot_id) = u32::try_from(args[1].clone().expect_u128()?) else {
        wrb_warn!("app slot is too big");
        env_with_global_context(
            global_context,
            sender,
            sponsor,
            wrb_lowlevel_contract.contract_context,
            |env| {
                env.execute_contract_allow_private(
                    contract_id,
                    "set-last-wrbpod-get-slice-result",
                    &[
                        SymbolicExpression::atom_value(Value::UInt(session_id)),
                        SymbolicExpression::atom_value(Value::UInt(args[1].clone().expect_u128()?)),
                        SymbolicExpression::atom_value(Value::UInt(slice_id)),
                        SymbolicExpression::atom_value(err_ascii_512(
                            WRB_ERR_INVALID,
                            "app slot is too big".into(),
                        )),
                    ],
                    false,
                )
            },
        )
        .expect("FATAL: failed to set last wrbpod-get-slice request");
        return Ok(());
    };

    let (name, namespace) = load_app_name(
        global_context,
        sender.clone(),
        sponsor.clone(),
        &wrb_lowlevel_contract,
    );

    // go fetch that slice
    let slice_res = with_globals(|globals| {
        let Some(wrbpod) = globals.get_wrbpod_session(session_id) else {
            wrb_warn!(
                "wrbpod.get_slice({}.{}, {}): no such session {}",
                &name,
                &namespace,
                app_slot_id,
                session_id
            );
            return Err("no such session".to_string());
        };
        wrbpod
            .get_slice(&format!("{}.{}", &name, &namespace), app_slot_id, slice_id)
            .ok_or_else(|| {
                wrb_warn!(
                    "wrbpod.get_slice({},{}): no such slice",
                    app_slot_id,
                    slice_id
                );
                format!("no such slice")
            })
    });

    let slice_res_value = match slice_res {
        Ok(bytes) => Value::okay(Value::buff_from(bytes).unwrap()).unwrap(),
        Err(msg) => err_ascii_512(WRB_ERR_WRBPOD_NO_SLICE, &msg),
    };

    env_with_global_context(
        global_context,
        sender,
        sponsor,
        wrb_lowlevel_contract.contract_context,
        |env| {
            env.execute_contract_allow_private(
                contract_id,
                "set-last-wrbpod-get-slice-result",
                &[
                    SymbolicExpression::atom_value(Value::UInt(session_id)),
                    SymbolicExpression::atom_value(Value::UInt(app_slot_id.into())),
                    SymbolicExpression::atom_value(Value::UInt(slice_id)),
                    SymbolicExpression::atom_value(slice_res_value),
                ],
                false,
            )
        },
    )
    .expect("FATAL: failed to set last wrbpod-get-slice request");
    Ok(())
}

/// Trampoline code for contract-call to `.wrb wrbpod-put-slice`
/// (define-public (wrbpod-put-slice (session-id uint) (slot-id uint) (slice-id uint) (slide-data (buff 786000))
/// returns (response bool (string-ascii 512))
pub fn handle_wrbpod_put_slice(
    global_context: &mut GlobalContext,
    sender: PrincipalData,
    sponsor: Option<PrincipalData>,
    contract_id: &QualifiedContractIdentifier,
    args: &[Value],
    wrb_lowlevel_contract: Contract,
) -> Result<(), Error> {
    // must be three arguments
    if args.len() != 4 {
        return Err(InterpreterError::InterpreterError(format!(
            "Expected 3 arguments, got {}",
            args.len()
        ))
        .into());
    }

    let session_id = args[0].clone().expect_u128()?;
    let slice_id = args[2].clone().expect_u128()?;
    let Ok(app_slot_id) = u32::try_from(args[1].clone().expect_u128()?) else {
        wrb_warn!("wrbpod-put-slice: app slot is too big");
        env_with_global_context(
            global_context,
            sender,
            sponsor,
            wrb_lowlevel_contract.contract_context,
            |env| {
                env.execute_contract_allow_private(
                    contract_id,
                    "set-last-wrbpod-put-slice-result",
                    &[
                        SymbolicExpression::atom_value(Value::UInt(session_id)),
                        SymbolicExpression::atom_value(Value::UInt(args[1].clone().expect_u128()?)),
                        SymbolicExpression::atom_value(Value::UInt(slice_id)),
                        SymbolicExpression::atom_value(err_ascii_512(
                            WRB_ERR_INVALID,
                            "app slot is too big".into(),
                        )),
                    ],
                    false,
                )
            },
        )
        .expect("FATAL: failed to set last wrbpod-put-slice request");
        return Ok(());
    };
    let slice_data = args[3].clone().expect_buff(786000)?;

    let (name, namespace) = load_app_name(
        global_context,
        sender.clone(),
        sponsor.clone(),
        &wrb_lowlevel_contract,
    );

    // go fetch that slice
    let put_res = with_globals(|globals| {
        let Some(wrbpod) = globals.get_wrbpod_session(session_id) else {
            wrb_warn!(
                "wrbpod-put-slice({}.{}, {}): no such session {}",
                &name,
                &namespace,
                app_slot_id,
                session_id
            );
            return Err("no such session".to_string());
        };
        Ok(wrbpod.put_slice(
            &format!("{}.{}", &name, &namespace),
            app_slot_id,
            slice_id,
            slice_data,
        ))
    });

    let put_res_value = match put_res {
        Ok(put_res) => Value::okay(Value::Bool(put_res)).unwrap(),
        Err(msg) => err_ascii_512(WRB_ERR_WRBPOD_PUT_SLICE_FAILURE, &msg),
    };

    env_with_global_context(
        global_context,
        sender,
        sponsor,
        wrb_lowlevel_contract.contract_context,
        |env| {
            env.execute_contract_allow_private(
                contract_id,
                "set-last-wrbpod-put-slice-result",
                &[
                    SymbolicExpression::atom_value(Value::UInt(session_id)),
                    SymbolicExpression::atom_value(Value::UInt(app_slot_id.into())),
                    SymbolicExpression::atom_value(Value::UInt(slice_id)),
                    SymbolicExpression::atom_value(put_res_value),
                ],
                false,
            )
        },
    )
    .expect("FATAL: failed to set last wrbpod-put-slice request");
    Ok(())
}

/// Trampoline code for `.wrb wrb-sync`
pub fn handle_wrbpod_sync_slot(
    global_context: &mut GlobalContext,
    sender: PrincipalData,
    sponsor: Option<PrincipalData>,
    contract_id: &QualifiedContractIdentifier,
    args: &[Value],
    wrb_lowlevel_contract: Contract,
) -> Result<(), Error> {
    // must be one arguments
    if args.len() != 2 {
        return Err(InterpreterError::InterpreterError(format!(
            "Expected 1 arguments, got {}",
            args.len()
        ))
        .into());
    }

    let session_id = args[0].clone().expect_u128()?;
    let Ok(app_slot_id) = u32::try_from(args[1].clone().expect_u128()?) else {
        wrb_warn!("app slot is too big");
        env_with_global_context(
            global_context,
            sender,
            sponsor,
            wrb_lowlevel_contract.contract_context,
            |env| {
                env.execute_contract_allow_private(
                    contract_id,
                    "set-last-wrbpod-sync-slot-result",
                    &[
                        SymbolicExpression::atom_value(Value::UInt(session_id)),
                        SymbolicExpression::atom_value(Value::UInt(args[1].clone().expect_u128()?)),
                        SymbolicExpression::atom_value(err_ascii_512(
                            WRB_ERR_INVALID,
                            "app slot is too big".into(),
                        )),
                    ],
                    false,
                )
            },
        )
        .expect("FATAL: failed to set last wrbpod-sync-slot request");
        return Ok(());
    };

    let (name, namespace) = load_app_name(
        global_context,
        sender.clone(),
        sponsor.clone(),
        &wrb_lowlevel_contract,
    );

    let res = with_globals(|globals| {
        let Some(wrbpod) = globals.get_wrbpod_session(session_id) else {
            wrb_warn!("wrbpod.sync: no such session {}", session_id);
            return Err("no such session".to_string());
        };
        wrbpod
            .sync_slot(&format!("{}.{}", &name, &namespace), app_slot_id)
            .map_err(|e| {
                wrb_warn!(
                    "Failed to put slot {}.{} {}: {:?}",
                    &name,
                    &namespace,
                    app_slot_id,
                    &e
                );
                format!("{:?}", &e)
            })
    });

    let res_val = match res {
        Ok(_) => Value::okay(Value::Bool(true)).unwrap(),
        Err(msg) => err_ascii_512(WRB_ERR_WRBPOD_SYNC_SLOT_FAILURE, &msg),
    };

    env_with_global_context(
        global_context,
        sender,
        sponsor,
        wrb_lowlevel_contract.contract_context,
        |env| {
            env.execute_contract_allow_private(
                contract_id,
                "set-last-wrbpod-sync-slot-result",
                &[
                    SymbolicExpression::atom_value(Value::UInt(session_id)),
                    SymbolicExpression::atom_value(Value::UInt(args[1].clone().expect_u128()?)),
                    SymbolicExpression::atom_value(res_val),
                ],
                false,
            )
        },
    )
    .expect("FATAL: failed to set last wrbpod-sync-slot request");
    Ok(())
}

pub fn handle_wrb_contract_call_special_cases(
    global_context: &mut GlobalContext,
    sender: Option<&PrincipalData>,
    sponsor: Option<&PrincipalData>,
    contract_id: &QualifiedContractIdentifier,
    function_name: &str,
    args: &[Value],
    result: &Value,
) -> Result<(), Error> {
    wrb_debug!(
        "Run special-case handler for {}.{}: {:?} --> {:?}",
        contract_id,
        function_name,
        args,
        result
    );
    if *contract_id == boot_code_id(WRB_LOW_LEVEL_CONTRACT, true) {
        let runner = make_runner();
        let sender = match sender {
            Some(s) => s.clone(),
            None => boot_code_addr(true).into(),
        };
        let sponsor = sponsor.cloned();
        let wrb_lowlevel_contract = global_context
            .database
            .get_contract(contract_id)
            .expect("FATAL: could not load wrb contract metadata");

        match function_name {
            "call-readonly" => {
                handle_wrb_call_readonly(
                    global_context,
                    sender,
                    sponsor,
                    contract_id,
                    args,
                    wrb_lowlevel_contract,
                    runner,
                )?;
            }
            "buff-to-string-utf8" => {
                handle_buff_to_string_utf8(
                    global_context,
                    sender,
                    sponsor,
                    contract_id,
                    args,
                    wrb_lowlevel_contract,
                )?;
            }
            "string-ascii-to-string-utf8" => {
                handle_string_ascii_to_string_utf8(
                    global_context,
                    sender,
                    sponsor,
                    contract_id,
                    args,
                    wrb_lowlevel_contract,
                )?;
            }
            "wrbpod-default" => {
                handle_wrbpod_default(
                    global_context,
                    sender,
                    sponsor,
                    contract_id,
                    args,
                    wrb_lowlevel_contract,
                )?;
            }
            "wrbpod-open" => {
                handle_wrbpod_open(
                    global_context,
                    sender,
                    sponsor,
                    contract_id,
                    args,
                    wrb_lowlevel_contract,
                )?;
            }
            "wrbpod-get-num-slots" => {
                handle_wrbpod_get_num_slots(
                    global_context,
                    sender,
                    sponsor,
                    contract_id,
                    args,
                    wrb_lowlevel_contract,
                )?;
            }
            "wrbpod-alloc-slots" => {
                handle_wrbpod_alloc_slots(
                    global_context,
                    sender,
                    sponsor,
                    contract_id,
                    args,
                    wrb_lowlevel_contract,
                )?;
            }
            "wrbpod-fetch-slot" => {
                handle_wrbpod_fetch_slot(
                    global_context,
                    sender,
                    sponsor,
                    contract_id,
                    args,
                    wrb_lowlevel_contract,
                )?;
            }
            "wrbpod-get-slice" => {
                handle_wrbpod_get_slice(
                    global_context,
                    sender,
                    sponsor,
                    contract_id,
                    args,
                    wrb_lowlevel_contract,
                )?;
            }
            "wrbpod-put-slice" => {
                handle_wrbpod_put_slice(
                    global_context,
                    sender,
                    sponsor,
                    contract_id,
                    args,
                    wrb_lowlevel_contract,
                )?;
            }
            "wrbpod-sync-slot" => {
                handle_wrbpod_sync_slot(
                    global_context,
                    sender,
                    sponsor,
                    contract_id,
                    args,
                    wrb_lowlevel_contract,
                )?;
            }
            _ => {}
        };
    }
    Ok(())
}
