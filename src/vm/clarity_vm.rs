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

use crate::util::{DEFAULT_CHAIN_ID, DEFAULT_WRB_CLARITY_VERSION, DEFAULT_WRB_EPOCH};
use clarity::boot_util::boot_code_addr;

use clarity::vm::analysis::AnalysisDatabase;
use clarity::vm::database::BurnStateDB;
use clarity::vm::database::ClarityDatabase;
use clarity::vm::database::HeadersDB;
use clarity::vm::eval_all;
use clarity::vm::ClarityVersion;
use clarity::vm::Value;
use clarity::vm::ContractContext;
use clarity::vm::database::MemoryBackingStore;
use clarity::vm::contexts::GlobalContext;
use clarity::vm::ast;
use crate::vm::storage;

use crate::vm::storage::util::*;
use crate::vm::storage::ReadOnlyWrbStore;
use crate::vm::storage::WritableWrbStore;
use crate::vm::storage::{WrbDB, WrbHeadersDB};
use crate::vm::ClarityStorage;
use crate::vm::ClarityVM;

use crate::vm::Error;
use crate::vm::WRB_LOW_LEVEL_CONTRACT;

use stacks_common::types::chainstate::StacksBlockId;
use stacks_common::util::hash::to_hex;
use stacks_common::util::hash::Hash160;
use stacks_common::address::{C32_ADDRESS_VERSION_MAINNET_SINGLESIG, C32_ADDRESS_VERSION_TESTNET_SINGLESIG};

use crate::core::with_global_config;

use crate::vm::BOOT_CODE;
use crate::vm::{BOOT_BLOCK_ID, GENESIS_BLOCK_ID};

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
    pub fn new(db_path: &str, domain: &str) -> Result<ClarityVM, Error> {
        let wrbdb = WrbDB::open(db_path, domain, None)?;
        let created = wrbdb.created();
        let mainnet = with_global_config(|cfg| cfg.mainnet())
            .ok_or(Error::NotInitialized)?;
        
        let mut parts = domain.split(".");
        let Some(name) = parts.next() else {
            return Err(Error::InvalidInput("Invalid BNS name".into()));
        };
        let Some(namespace) = parts.next() else {
            return Err(Error::InvalidInput("Invalid BNS name".into()));
        };
        if parts.next().is_some() {
            return Err(Error::InvalidInput("Invalid BNS name".into()));
        }

        let mut vm = ClarityVM {
            db: wrbdb,
            mainnet,
            app_name: name.to_string(),
            app_namespace: namespace.to_string(),
        };

        if created {
            vm.install_boot_code(BOOT_CODE)?;
        }
        Ok(vm)
    }

    /// Start working on the next iteration of loading up the wrb page
    pub fn begin_page_load<'a>(&'a mut self, code_hash: &Hash160) -> Result<WritableWrbStore<'a>, Error> {
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
        
        let mainnet = self.mainnet;
        let ll_contract_id = QualifiedContractIdentifier::new(
            boot_code_addr(mainnet).into(),
            ContractName::try_from(WRB_LOW_LEVEL_CONTRACT).unwrap(),
        );
        let headers_db = self.db.headers_db();
        let mut write_tx = self.db.begin(&cur_tip, &next_tip);
        let mut db = write_tx.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);

        db.begin();
        let mut vm_env =
            OwnedEnvironment::new_free(mainnet, DEFAULT_CHAIN_ID, db, DEFAULT_WRB_EPOCH);
        let (contract, _, _) = vm_env.execute_in_env(
            StandardPrincipalData::transient().into(),
            None,
            None,
            |env| env.global_context.database.get_contract(&ll_contract_id),
        )?;

        let code = format!(r#"(set-app-code-hash 0x{})"#, code_hash);
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
        Ok(write_tx)
    }

    /// Start working on the next iteration of loading up the wrb page, but in a read-only manner
    pub fn begin_read_only<'a>(&'a mut self) -> ReadOnlyWrbStore<'a> {
        let cur_tip = get_wrb_chain_tip(self.db.conn());
        self.db.begin_read_only(Some(&cur_tip))
    }

    /// Get the code ID for the page
    pub fn get_code_id(&self) -> QualifiedContractIdentifier {
        let hash = Hash160::from_data(&self.db.get_domain().as_bytes());
        let version = if self.mainnet {
            C32_ADDRESS_VERSION_MAINNET_SINGLESIG
        }
        else {
            C32_ADDRESS_VERSION_TESTNET_SINGLESIG
        };
        QualifiedContractIdentifier::new(StandardPrincipalData(version, hash.0), "main".into())
    }

    /// Does there exist a code body with this ID?
    pub fn has_code(&mut self, code_id: &QualifiedContractIdentifier) -> Result<bool, Error> {
        let headers_db = self.db.headers_db();
        let mut wrb_read = self.begin_read_only();
        let mut db = wrb_read.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);
        db.begin();
        let res = db.has_contract(code_id);
        db.roll_back()?;
        Ok(res)
    }

    /// Instantiate a HeadersDB
    pub fn headers_db(&self) -> WrbHeadersDB {
        self.db.headers_db()
    }

    /// Set up the wrb boot code
    fn install_boot_code(&mut self, boot_code: &[(&str, &str)]) -> Result<(), Error> {
        let mainnet = self.mainnet;
        let name = self.app_name.clone();
        let namespace = self.app_namespace.clone();

        let headers_db = self.headers_db();
        let mut write_tx = self.db.begin(&BOOT_BLOCK_ID, &GENESIS_BLOCK_ID);

        for (boot_code_name, boot_code_contract) in boot_code.iter() {
            let contract_identifier = QualifiedContractIdentifier::new(
                boot_code_addr(mainnet).into(),
                ContractName::try_from(boot_code_name.to_string()).unwrap(),
            );
            let contract_content = *boot_code_contract;

            // sanity check -- don't do this more than once
            let mut db = write_tx.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);
            db.begin();
            let has_contract = db.has_contract(&contract_identifier);
            db.roll_back()?;

            if has_contract {
                continue;
            }

            wrb_debug!(
                "Instantiate boot code contract '{}' ({} bytes)...",
                &contract_identifier,
                boot_code_contract.len()
            );

            let mut ast =
                parse(&contract_identifier, &contract_content).expect("Failed to parse program");

            wrb_debug!("Analyze contract {}", &contract_identifier);
            run_analysis_free(&contract_identifier, &mut ast, &mut write_tx, true).expect(
                &format!("FATAL: failed to analyze {}", &contract_identifier),
            );

            wrb_debug!("Deploy contract {}", &contract_identifier);
            let mut db = write_tx.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);
            db.begin();
            let mut vm_env =
                OwnedEnvironment::new_free(mainnet, DEFAULT_CHAIN_ID, db, DEFAULT_WRB_EPOCH);
            vm_env
                .initialize_versioned_contract(
                    contract_identifier.clone(),
                    DEFAULT_WRB_CLARITY_VERSION,
                    &contract_content,
                    None,
                    ASTRules::PrecheckSize,
                )
                .expect(&format!(
                    "FATAL: failed to initialize boot contract '{}'",
                    &contract_identifier
                ));

            let (mut db, _) = vm_env
                .destruct()
                .expect("Failed to recover database reference after executing transaction");

            db.commit()?;
        }

        // set domain name and code hash
        wrb_debug!("Set app name to {}.{}", name, namespace);

        let ll_contract_id = QualifiedContractIdentifier::new(
            boot_code_addr(mainnet).into(),
            ContractName::try_from(WRB_LOW_LEVEL_CONTRACT).unwrap(),
        );

        let mut db = write_tx.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);
        db.begin();
        let mut vm_env =
            OwnedEnvironment::new_free(mainnet, DEFAULT_CHAIN_ID, db, DEFAULT_WRB_EPOCH);
        let (contract, _, _) = vm_env.execute_in_env(
            StandardPrincipalData::transient().into(),
            None,
            None,
            |env| env.global_context.database.get_contract(&ll_contract_id),
        )?;

        let code = format!(r#"(set-app-name {{ name: 0x{}, namespace: 0x{} }})"#, to_hex(name.as_bytes()), to_hex(namespace.as_bytes()));
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

        write_tx.commit_to(&GENESIS_BLOCK_ID)?;
        Ok(())
    }
}
