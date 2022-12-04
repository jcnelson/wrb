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

use std::error;
use std::io::Error as io_error;
use std::fmt;
use std::convert::From;

use rusqlite::Error as sqlite_error;
use rusqlite::Connection;
use clarity::{
    vm::analysis,
    vm::analysis::{errors::CheckError, ContractAnalysis},
    vm::ast::ASTRules,
    vm::ast::build_ast_with_rules,
    vm::contexts::{OwnedEnvironment},
    vm::costs::LimitedCostTracker,
    vm::database::{
        NULL_BURN_STATE_DB,
    },
    vm::errors::{Error as ClarityVMError, RuntimeErrorType},
    vm::types::{QualifiedContractIdentifier, StandardPrincipalData},
    vm::ContractName,
    vm::{SymbolicExpression},
    vm::representations::ClarityName,
};

use stacks_common::util::log;

use clarity::boot_util::boot_code_addr;
use crate::util::{DEFAULT_WRB_EPOCH, DEFAULT_WRB_CLARITY_VERSION, DEFAULT_CHAIN_ID};

use clarity::vm::database::HeadersDB;
use clarity::vm::database::BurnStateDB;
use clarity::vm::database::ClarityDatabase;
use clarity::vm::analysis::AnalysisDatabase;

use crate::storage;

use crate::vm::ClarityStorage;
use crate::vm::ClarityVM;
use crate::storage::ReadOnlyWrbStore;
use crate::storage::WritableWrbStore;
use crate::storage::{WrbDB, WrbHeadersDB};
use crate::storage::util::*;

use stacks_common::types::chainstate::StacksBlockId;

use stacks_common::util::hash::Hash160;

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
        DEFAULT_WRB_CLARITY_VERSION,
    )
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
    pub fn new(db_path: &str, domain: &str) -> Result<ClarityVM, storage::Error> {
        let wrbdb = WrbDB::open(db_path, domain, None)?;
        let mut vm = ClarityVM {
            db: wrbdb
        };

        vm.install_boot_code(BOOT_CODE);
        Ok(vm)
    }

    /// Start working on the next iteration of loading up the wrb page
    pub fn begin_page_load<'a>(&'a mut self) -> WritableWrbStore<'a> {
        let cur_tip = get_wrb_chain_tip(self.db.conn());
        let cur_height = get_wrb_block_height(self.db.conn(), &cur_tip)
            .expect(&format!("FATAL: failed to determine height of {}", &cur_tip));
        let next_tip = make_wrb_chain_tip(cur_height + 1);

        debug!("Begin page load {},{} -> {},{}", &cur_tip, cur_height, &next_tip, cur_height + 1);
        self.db.begin(&cur_tip, &next_tip)
    }
    
    /// Start working on the next iteration of loading up the wrb page, but in a read-only manner
    pub fn begin_read_only<'a>(&'a mut self) -> ReadOnlyWrbStore<'a> {
        let cur_tip = get_wrb_chain_tip(self.db.conn());
        self.db.begin_read_only(Some(&cur_tip))
    }

    /// Get the code ID for the page
    pub fn get_code_id(&self) -> QualifiedContractIdentifier {
        let hash = Hash160::from_data(&self.db.get_domain().as_bytes());
        QualifiedContractIdentifier::new(StandardPrincipalData(1, hash.0), "main".into())
    }

    /// Does there exist a code body with this ID?
    pub fn has_code(&mut self, code_id: &QualifiedContractIdentifier) -> bool {
        let headers_db = self.db.headers_db();
        let mut wrb_read = self.begin_read_only();
        let mut db = wrb_read.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);
        db.begin();
        let res = db.has_contract(code_id);
        db.roll_back();
        res
    }

    /// Instantiate a HeadersDB
    pub fn headers_db(&self) -> WrbHeadersDB {
        self.db.headers_db()
    }

    /// Set up the wrb boot code
    fn install_boot_code(&mut self, boot_code: &[(&str, &str)]) {
        let headers_db = self.headers_db();
        let mut write_tx = self.db.begin(&BOOT_BLOCK_ID, &GENESIS_BLOCK_ID);

        for (boot_code_name, boot_code_contract) in boot_code.iter() {
            let contract_identifier = QualifiedContractIdentifier::new(
                boot_code_addr(true).into(),
                ContractName::try_from(boot_code_name.to_string()).unwrap(),
            );
            let contract_content = *boot_code_contract;

            debug!(
                "Instantiate boot code contract '{}' ({} bytes)...",
                &contract_identifier,
                boot_code_contract.len()
            );

            let mut ast = parse(&contract_identifier, &contract_content)
                .expect("Failed to parse program");

            let analysis_result = run_analysis_free(&contract_identifier, &mut ast, &mut write_tx, true);
            match analysis_result {
                Ok(_) => {
                    let db = write_tx.get_clarity_db(&headers_db, &NULL_BURN_STATE_DB);
                    let mut vm_env = OwnedEnvironment::new_free(
                        true,
                        DEFAULT_CHAIN_ID,
                        db,
                        DEFAULT_WRB_EPOCH,
                    );
                    vm_env
                        .initialize_versioned_contract(
                            contract_identifier.clone(),
                            DEFAULT_WRB_CLARITY_VERSION,
                            &contract_content,
                            None,
                            ASTRules::PrecheckSize
                        )
                        .expect(&format!("FATAL: failed to initialize boot contract '{}'", &contract_identifier));
                }
                Err(_) => {
                    panic!("failed to instantiate boot contract");
                }
            };
        }
        write_tx.commit_to(&GENESIS_BLOCK_ID);
    }
}

