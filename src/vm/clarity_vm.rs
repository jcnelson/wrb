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

use std::convert::From;
use std::error;
use std::fmt;
use std::io::Error as io_error;

use sha2::{Digest, Sha256};

use clarity::{
    vm::analysis,
    vm::analysis::{errors::CheckError, ContractAnalysis},
    vm::ast::build_ast_with_rules,
    vm::ast::ASTRules,
    vm::contexts::OwnedEnvironment,
    vm::costs::LimitedCostTracker,
    vm::database::NULL_BURN_STATE_DB,
    vm::errors::{Error as ClarityVMError, RuntimeErrorType},
    vm::representations::ClarityName,
    vm::types::{QualifiedContractIdentifier, StandardPrincipalData},
    vm::ContractName,
    vm::SymbolicExpression,
};
use rusqlite::Connection;
use rusqlite::Error as sqlite_error;

use stacks_common::util::log;

use crate::vm::BOOT_BLOCK_ID;
use crate::vm::GENESIS_BLOCK_ID;

use crate::util::{DEFAULT_CHAIN_ID, DEFAULT_WRB_CLARITY_VERSION, DEFAULT_WRB_EPOCH};
use clarity::boot_util::boot_code_addr;

use crate::vm::storage;
use clarity::vm::analysis::AnalysisDatabase;
use clarity::vm::ast;
use clarity::vm::contexts::GlobalContext;
use clarity::vm::database::BurnStateDB;
use clarity::vm::database::ClarityDatabase;
use clarity::vm::database::HeadersDB;
use clarity::vm::database::MemoryBackingStore;
use clarity::vm::eval_all;
use clarity::vm::ClarityVersion;
use clarity::vm::ContractContext;
use clarity::vm::Value;

use crate::vm::storage::util::*;
use crate::vm::storage::ReadOnlyWrbStore;
use crate::vm::storage::WritableWrbStore;
use crate::vm::storage::{WrbDB, WrbHeadersDB};
use crate::vm::ClarityStorage;
use crate::vm::ClarityVM;

use crate::vm::Error;

use stacks_common::types::chainstate::StacksBlockId;
use stacks_common::util::hash::to_hex;
use stacks_common::util::hash::Hash160;
use stacks_common::util::hash::Sha256Sum;

use crate::core::split_fqn;
use crate::core::with_global_config;

use crate::vm::contracts::WRB_LL_CODE;
use crate::vm::wrb_link_app;

/// Parse contract code, given the identifier.
/// TODO: add pass(es) to remove unusable Clarity keywords
pub fn parse(
    contract_identifier: &QualifiedContractIdentifier,
    source_code: &str,
) -> Result<Vec<SymbolicExpression>, ClarityVMError> {
    let ast = build_ast_with_rules(
        contract_identifier,
        source_code,
        &mut (),
        DEFAULT_WRB_CLARITY_VERSION,
        DEFAULT_WRB_EPOCH,
        ASTRules::PrecheckSize,
    )
    .map_err(|e| RuntimeErrorType::ASTError(e))?;
    Ok(ast.expressions)
}

/// Analyze parsed contract code, without cost limits
pub fn run_analysis_free<C: ClarityStorage>(
    contract_identifier: &QualifiedContractIdentifier,
    expressions: &mut [SymbolicExpression],
    clarity_kv: &mut C,
    save_contract: bool,
) -> Result<ContractAnalysis, (CheckError, LimitedCostTracker)> {
    analysis::run_analysis(
        contract_identifier,
        expressions,
        &mut clarity_kv.get_analysis_db(),
        save_contract,
        LimitedCostTracker::new_free(),
        DEFAULT_WRB_EPOCH,
        DEFAULT_WRB_CLARITY_VERSION,
        false,
    )
}

/// Execute program in a transient environment.
pub fn vm_execute(program: &str, clarity_version: ClarityVersion) -> Result<Option<Value>, Error> {
    let contract_id = QualifiedContractIdentifier::transient();
    let mut contract_context = ContractContext::new(contract_id.clone(), clarity_version);
    let mut marf = MemoryBackingStore::new();
    let conn = marf.as_clarity_db();
    let mut global_context = GlobalContext::new(
        true,
        DEFAULT_CHAIN_ID,
        conn,
        LimitedCostTracker::new_free(),
        DEFAULT_WRB_EPOCH,
    );
    Ok(global_context.execute(|g| {
        let parsed = ast::build_ast_with_rules(
            &contract_id,
            program,
            &mut (),
            clarity_version,
            DEFAULT_WRB_EPOCH,
            ASTRules::PrecheckSize,
        )?
        .expressions;
        eval_all(&parsed, &mut contract_context, g, None)
    })?)
}

impl ClarityStorage for WritableWrbStore<'_> {
    fn get_clarity_db<'a>(
        &'a mut self,
        headers_db: &'a dyn HeadersDB,
        burn_db: &'a dyn BurnStateDB,
    ) -> ClarityDatabase<'a> {
        self.as_clarity_db(headers_db, burn_db)
    }

    fn get_analysis_db<'a>(&'a mut self) -> AnalysisDatabase<'a> {
        self.as_analysis_db()
    }
}

impl ClarityStorage for ReadOnlyWrbStore<'_> {
    fn get_clarity_db<'a>(
        &'a mut self,
        headers_db: &'a dyn HeadersDB,
        burn_db: &'a dyn BurnStateDB,
    ) -> ClarityDatabase<'a> {
        self.as_clarity_db(headers_db, burn_db)
    }

    fn get_analysis_db<'a>(&'a mut self) -> AnalysisDatabase<'a> {
        self.as_analysis_db()
    }
}

impl ClarityVM {
    pub fn new(db_path: &str, domain: &str, version: u32) -> Result<ClarityVM, Error> {
        let wrbdb = WrbDB::open(db_path, domain, None)?;
        let (name, namespace) = split_fqn(domain).map_err(|e_str| Error::InvalidInput(e_str))?;

        let vm = ClarityVM {
            db: wrbdb,
            app_name: name.to_string(),
            app_namespace: namespace.to_string(),
            app_version: version,
        };
        Ok(vm)
    }

    /// Get the code hash (hash of compressed bytes and version)
    fn get_code_hash(&self, compressed_bytes: &[u8]) -> Hash160 {
        let mut h = Sha256::new();
        h.update(compressed_bytes);
        h.update(&self.app_version.to_be_bytes());

        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(h.finalize().as_slice());

        Hash160::from_sha256(&bytes)
    }

    /// Start working on the next iteration of loading up the wrb page
    pub fn begin_page_load<'a>(&'a mut self) -> Result<WritableWrbStore<'a>, Error> {
        let cur_tip = get_wrb_chain_tip(self.db.conn());
        let cur_height = get_wrb_block_height(self.db.conn(), &cur_tip).expect(&format!(
            "FATAL: failed to determine height of {}",
            &cur_tip
        ));
        let next_tip = make_wrb_chain_tip(cur_height + 1);

        wrb_debug!(
            "Begin page load {},{} -> {},{}",
            &cur_tip,
            cur_height,
            &next_tip,
            cur_height + 1
        );

        let write_tx = self.db.begin(&cur_tip, &next_tip);
        Ok(write_tx)
    }

    /// Start working on the next iteration of loading up the wrb page, but in a read-only manner
    pub fn begin_read_only<'a>(&'a mut self) -> ReadOnlyWrbStore<'a> {
        let cur_tip = get_wrb_chain_tip(self.db.conn());
        self.db.begin_read_only(Some(&cur_tip))
    }

    /// Instantiate a HeadersDB
    pub fn headers_db(&self) -> WrbHeadersDB {
        self.db.headers_db()
    }

    /// Set up the wrb application
    pub fn initialize_app(&mut self, app_code: &str) -> Result<QualifiedContractIdentifier, Error> {
        let name = self.app_name.clone();
        let namespace = self.app_namespace.clone();
        let version = self.app_version;

        let linked_app_code = wrb_link_app(app_code);
        let code_hash = self.get_code_hash(linked_app_code.as_bytes());

        let app_contract_id = QualifiedContractIdentifier::new(
            boot_code_addr(true).into(),
            ContractName::try_from(format!("{}-{}-{}", name, namespace, version).as_str())
                .map_err(|e| {
                    Error::Clarity(format!(
                        "Invalid contract name '{}-{}-{}: {:?}",
                        &name, &namespace, version, &e
                    ))
                })?,
        );

        let ll_contract_id = QualifiedContractIdentifier::new(
            boot_code_addr(true).into(),
            ContractName::try_from("wrb-ll".to_string()).unwrap(),
        );

        let headers_db = self.headers_db();
        let mut write_tx = self.db.begin(&BOOT_BLOCK_ID, &GENESIS_BLOCK_ID);

        // sanity check -- don't do this more than once
        let mut db = write_tx.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);
        db.begin();
        let has_ll_contract = db.has_contract(&ll_contract_id);
        let has_app_contract = db.has_contract(&app_contract_id);
        db.roll_back()?;

        if !has_ll_contract {
            wrb_debug!(
                "Instantiate wrb-ll code to contract '{}' ({} bytes)...",
                &app_contract_id,
                linked_app_code.len(),
            );

            let mut ast = parse(&ll_contract_id, &WRB_LL_CODE)?;

            wrb_debug!("Analyze wrb-ll contract {}", &ll_contract_id);
            run_analysis_free(&ll_contract_id, &mut ast, &mut write_tx, true)
                .map_err(|(e, _)| Error::Clarity(format!("Analysis: {:?}", &e)))?;

            let mut db = write_tx.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);
            db.begin();
            let mut vm_env =
                OwnedEnvironment::new_free(true, DEFAULT_CHAIN_ID, db, DEFAULT_WRB_EPOCH);

            wrb_debug!("Deploy wrb-ll contract {}", &ll_contract_id);
            vm_env.initialize_versioned_contract(
                ll_contract_id.clone(),
                DEFAULT_WRB_CLARITY_VERSION,
                &WRB_LL_CODE,
                None,
                ASTRules::PrecheckSize,
            )?;

            // set domain name and code hash
            wrb_debug!("Set app name to {}.{} version {}", name, namespace, version);
            let (contract, _, _) = vm_env.execute_in_env(
                StandardPrincipalData::transient().into(),
                None,
                None,
                |env| env.global_context.database.get_contract(&ll_contract_id),
            )?;

            let code = format!(
                r#"(wrb-ll-set-app-name {{ name: 0x{}, namespace: 0x{}, version: u{} }})"#,
                to_hex(name.as_bytes()),
                to_hex(namespace.as_bytes()),
                version
            );
            vm_env.execute_in_env(
                StandardPrincipalData::transient().into(),
                None,
                Some(contract.contract_context.clone()),
                |env| env.eval_raw_with_rules(&code, ASTRules::PrecheckSize),
            )?;

            wrb_debug!("Set app code hash to {}", &code_hash);
            let code = format!(r#"(wrb-ll-set-app-code-hash 0x{})"#, code_hash);
            vm_env.execute_in_env(
                StandardPrincipalData::transient().into(),
                None,
                Some(contract.contract_context),
                |env| env.eval_raw_with_rules(&code, ASTRules::PrecheckSize),
            )?;

            let (mut db, _) = vm_env
                .destruct()
                .expect("Failed to recover database reference after executing transaction");

            db.commit()?;
        }

        if !has_app_contract {
            wrb_debug!(
                "Instantiate app code to contract '{}' ({} bytes)...",
                &app_contract_id,
                linked_app_code.len(),
            );

            let mut ast = parse(&app_contract_id, &linked_app_code)?;

            wrb_debug!("Analyze linked app contract {}", &app_contract_id);
            run_analysis_free(&app_contract_id, &mut ast, &mut write_tx, true)
                .map_err(|(e, _)| Error::Clarity(format!("Analysis: {:?}", &e)))?;

            let mut db = write_tx.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);
            db.begin();
            let mut vm_env =
                OwnedEnvironment::new_free(true, DEFAULT_CHAIN_ID, db, DEFAULT_WRB_EPOCH);

            wrb_debug!("Deploy linked app contract {}", &app_contract_id);
            vm_env.initialize_versioned_contract(
                app_contract_id.clone(),
                DEFAULT_WRB_CLARITY_VERSION,
                &linked_app_code,
                None,
                ASTRules::PrecheckSize,
            )?;

            let (mut db, _) = vm_env
                .destruct()
                .expect("Failed to recover database reference after executing transaction");
            db.commit()?;
        }

        write_tx.commit_to(&GENESIS_BLOCK_ID)?;

        wrb_debug!("Initialized app code to contract '{}'", &app_contract_id);

        Ok(app_contract_id)
    }
}
