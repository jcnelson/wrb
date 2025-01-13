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

use std::collections::HashMap;
use std::path::PathBuf;

use rand::Rng;

use rusqlite::Connection;
use rusqlite::OpenFlags;
use rusqlite::ToSql;
use rusqlite::Transaction;

use clarity::vm::analysis::AnalysisDatabase;
use clarity::vm::database::sqlite::{
    sqlite_get_contract_hash, sqlite_get_metadata, sqlite_get_metadata_manual,
    sqlite_insert_metadata,
};
use clarity::vm::database::ClarityBackingStore;
use clarity::vm::database::SpecialCaseHandler;
use clarity::vm::database::{BurnStateDB, ClarityDatabase, HeadersDB, SqliteConnection};
use clarity::vm::errors::{Error as ClarityError, RuntimeErrorType};
use clarity::vm::types::QualifiedContractIdentifier;
use stacks_common::types::chainstate::BlockHeaderHash;
use stacks_common::types::chainstate::StacksBlockId;
use stacks_common::types::chainstate::TrieHash;
use stacks_common::util::hash::Sha512Trunc256Sum;

use crate::util::sqlite::{
    query_row, sqlite_open, tx_begin_immediate, u64_to_sql, Error as db_error,
};
use crate::vm::storage::util::*;
use crate::vm::storage::Error;
use crate::vm::storage::ReadOnlyWrbStore;
use crate::vm::storage::WrbDB;
use crate::vm::storage::WrbHeadersDB;
use crate::vm::storage::WritableWrbStore;
use crate::vm::storage::WriteBuffer;

use crate::vm::special::handle_wrb_contract_call_special_cases;
use crate::vm::{BOOT_BLOCK_ID, GENESIS_BLOCK_ID};

use clarity::vm::errors::Error as clarity_error;

const SCHEMA_VERSION: &'static str = "1";

const KV_SCHEMA: &'static [&'static str] = &[
    r#"
    CREATE TABLE IF NOT EXISTS kvstore(
        chain_tip TEXT NOT NULL,
        height INTEGER NOT NULL,
        key TEXT NOT NULL,
        data_hash TEXT NOT NULL,
        PRIMARY KEY(chain_tip, height, key)
    );
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS schema_version(
        version INTEGER NOT NULL
    );
    "#,
    r#"
    INSERT INTO schema_version (version) VALUES (1);
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS wrb_config(
        mainnet BOOLEAN NOT NULL
    );
    "#,
];

/// Get the height of a block hash, without regards to the open chain tip
fn tipless_get_block_height_of(
    conn: &Connection,
    bhh: &StacksBlockId,
) -> Result<Option<u64>, Error> {
    if bhh == &BOOT_BLOCK_ID {
        return Ok(Some(0));
    }
    let tip_opt = query_row::<u64, _>(
        conn,
        "SELECT height FROM kvstore WHERE chain_tip = ?1",
        &[bhh as &dyn ToSql],
    )
    .map_err(|e| Error::DBError(e.into()))?;

    if let Some(tip_height) = &tip_opt {
        if bhh != &GENESIS_BLOCK_ID {
            assert!(*tip_height > 0, "BUG: bhh = {} height {}", bhh, tip_height);
        }
    }
    Ok(tip_opt)
}

/// Check to see if the block hash exists in the K/V
fn check_block_hash(
    conn: &Connection,
    tip_height: u64,
    bhh: &StacksBlockId,
) -> Result<bool, Error> {
    match query_row::<i64, _>(
        conn,
        "SELECT 1 FROM kvstore WHERE chain_tip = ?1 AND height <= ?2",
        &[bhh as &dyn ToSql, &u64_to_sql(tip_height)?],
    ) {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// Get the height of a block hash
fn get_block_height_of(
    conn: &Connection,
    tip_height: u64,
    bhh: &StacksBlockId,
) -> Result<Option<u64>, Error> {
    query_row::<u64, _>(
        conn,
        "SELECT height FROM kvstore WHERE chain_tip = ?1 AND height <= ?2",
        &[bhh as &dyn ToSql, &u64_to_sql(tip_height)?],
    )
    .map_err(|e| e.into())
}

/// Get the hash of a block given a height
fn get_block_at_height(
    conn: &Connection,
    tip_height: u64,
    height: u64,
) -> Result<Option<StacksBlockId>, Error> {
    if tip_height < height {
        return Ok(None);
    }
    query_row::<StacksBlockId, _>(
        conn,
        "SELECT chain_tip FROM kvstore WHERE height = ?1",
        &[&u64_to_sql(height)?],
    )
    .map_err(|e| e.into())
}

/// Get the hash of a value given the block hash and tip height
fn get_hash(conn: &Connection, tip_height: u64, key: &str) -> Result<Option<String>, Error> {
    let args: &[&dyn ToSql] = &[&key.to_string(), &u64_to_sql(tip_height)?];
    query_row::<String, _>(
        conn,
        "SELECT data_hash FROM kvstore WHERE key = ?1 AND height <= ?2 ORDER BY height DESC",
        args,
    )
    .map_err(|e| e.into())
}

/// Get the highest height stored
fn tipless_get_highest_tip_height(conn: &Connection) -> Result<u64, Error> {
    query_row::<u64, _>(
        conn,
        "SELECT IFNULL(MAX(height),0) FROM kvstore",
        rusqlite::params![],
    )
    .map_err(|e| e.into())
    .and_then(|height_opt| Ok(height_opt.unwrap_or(0)))
}

impl WrbHeadersDB {
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

impl WrbDB {
    fn setup_db(path_str: &str, domain: &str) -> Result<(Connection, bool), Error> {
        let mut path = PathBuf::from(path_str);
        path.push(domain);

        std::fs::create_dir_all(&path)?;

        path.push("db.sqlite");
        let (create, open_flags) = if std::fs::metadata(&path).is_ok() {
            (false, OpenFlags::SQLITE_OPEN_READ_WRITE)
        } else {
            (
                true,
                OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_READ_WRITE,
            )
        };

        let mut conn = sqlite_open(&path, open_flags, true)?;

        if SqliteConnection::check_schema(&conn).is_ok() {
            // no need to initialize
            return Ok((conn, false));
        }

        if !create {
            return Ok((conn, false));
        }

        let tx = tx_begin_immediate(&mut conn)?;

        SqliteConnection::initialize_conn(&tx).map_err(|e| {
            wrb_error!("Failed to initialize DB: {:?}", &e);
            Error::InitializationFailure
        })?;

        wrb_debug!("Instantiate WrbDB for {} at {}", domain, path_str);

        for cmd in KV_SCHEMA.iter() {
            tx.execute(cmd, rusqlite::params![])?;
        }

        // sentinel boot block state
        let values: &[&dyn ToSql] = &[
            &BOOT_BLOCK_ID,
            &0,
            &"genesis".to_string(),
            &format!("{}", &BOOT_BLOCK_ID),
        ];
        tx.execute(
            "REPLACE INTO kvstore (chain_tip, height, key, data_hash) VALUES (?1, ?2, ?3, ?4)",
            values,
        )
        .map_err(|e| Error::DBError(db_error::SqliteError(e)))?;

        tx.commit()?;
        Ok((conn, true))
    }

    fn open_readonly(path_str: &str, domain: &str) -> Result<Connection, Error> {
        let mut path = PathBuf::from(path_str);
        path.push(domain);
        path.push("db.sqlite");

        let conn = sqlite_open(&path, OpenFlags::SQLITE_OPEN_READ_ONLY, true)?;
        Ok(conn)
    }

    pub fn open(
        path_str: &str,
        domain: &str,
        chain_tip: Option<&StacksBlockId>,
    ) -> Result<WrbDB, Error> {
        let (conn, created) = WrbDB::setup_db(path_str, domain)?;
        let chain_tip = match chain_tip {
            Some(ref tip) => (*tip).clone(),
            None => {
                let height = tipless_get_highest_tip_height(&conn)?;
                if let Some(bhh) = get_block_at_height(&conn, height, height)? {
                    bhh
                } else {
                    BOOT_BLOCK_ID.clone()
                }
            }
        };

        Ok(WrbDB {
            db_path: path_str.to_string(),
            domain: domain.to_string(),
            conn,
            chain_tip,
            mainnet: true,
            created,
        })
    }

    /// Get a ref to the inner connection
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Get the domain
    pub fn get_domain(&self) -> &str {
        &self.domain
    }

    /// Did we create this DB when we opened it?
    pub fn created(&self) -> bool {
        self.created
    }

    /// Create a headers DB for this DB
    pub fn headers_db(&self) -> WrbHeadersDB {
        let conn = Self::open_readonly(&self.db_path, &self.domain)
            .expect("FATAL: could not open read only");

        WrbHeadersDB { conn }
    }

    /// Begin a read-only session at a particular point in time.
    pub fn begin_read_only<'a>(&'a self, at_block: Option<&StacksBlockId>) -> ReadOnlyWrbStore<'a> {
        let chain_tip = at_block
            .map(|b| (*b).clone())
            .unwrap_or(self.chain_tip.clone());
        let tip_height = tipless_get_block_height_of(&self.conn, &chain_tip)
            .expect("FATAL: DB error")
            .expect(&format!("FATAL: do not have height for tip {}", &chain_tip));

        ReadOnlyWrbStore {
            chain_tip,
            tip_height,
            conn: &self.conn,
            mainnet: self.mainnet,
        }
    }

    /// begin, commit, rollback a save point identified by key
    ///    this is used to clean up any data from aborted blocks
    ///     (NOT aborted transactions; that is handled by the clarity vm directly).
    /// The block header hash is used for identifying savepoints.
    ///     this _cannot_ be used to rollback to arbitrary prior block hash, because that
    ///     blockhash would already have committed and no longer exist in the save point stack.
    /// this is a "lower-level" rollback than the roll backs performed in
    ///   ClarityDatabase or AnalysisDatabase -- this is done at the backing store level.
    pub fn begin<'a>(
        &'a mut self,
        current: &StacksBlockId,
        next: &StacksBlockId,
    ) -> WritableWrbStore<'a> {
        let tx = tx_begin_immediate(&mut self.conn).expect("FATAL: could not begin transaction");

        let tip_height = tipless_get_block_height_of(&tx, current)
            .expect("DB failure")
            .expect(&format!("FATAL: do not have height for tip {}", current));

        let current_tip_height = if current == &BOOT_BLOCK_ID {
            0
        } else {
            get_block_height_of(&tx, tip_height, current)
                .expect("DB failure")
                .expect(&format!(
                    "FATAL: given tip {} has no known block height as of tip {} (height {})",
                    current, current, tip_height
                ))
                + 1
        };

        WritableWrbStore {
            chain_tip: current.clone(),
            tip_height: current_tip_height,
            next_tip: next.clone(),
            write_buf: WriteBuffer::new(),
            tx: tx,
            mainnet: self.mainnet,
        }
    }

    pub fn get_chain_tip(&self) -> &StacksBlockId {
        &self.chain_tip
    }

    pub fn set_chain_tip(&mut self, bhh: &StacksBlockId) {
        self.chain_tip = bhh.clone();
    }
}

impl WriteBuffer {
    pub fn new() -> WriteBuffer {
        WriteBuffer {
            pending_hashes: vec![],
            pending_data: HashMap::new(),
            pending_index: HashMap::new(),
        }
    }

    pub fn from_kv(keys: &Vec<(String, String)>) -> WriteBuffer {
        let mut wb = WriteBuffer::new();
        for (k, v) in keys.into_iter() {
            wb.put(&k, &v)
        }
        wb
    }

    pub fn get(&self, key: &str) -> Option<String> {
        if let Some(idx) = self.pending_index.get(&key.to_string()) {
            let hash = &self.pending_hashes[*idx].1;
            self.pending_data.get(hash).cloned()
        } else {
            None
        }
    }

    pub fn put(&mut self, key: &str, value: &str) {
        let k = key.to_string();
        let hash = Sha512Trunc256Sum::from_data(value.as_bytes());
        let hash_hex = hash.to_hex();
        self.pending_hashes.push((k.clone(), hash_hex.clone()));
        self.pending_index.insert(k, self.pending_hashes.len());
        self.pending_data.insert(hash_hex, value.to_string());
    }

    pub fn dump(
        &mut self,
        conn: &Connection,
        cur_tip: &StacksBlockId,
        next_tip: &StacksBlockId,
    ) -> Result<(), Error> {
        let next_height = match get_wrb_block_height(conn, cur_tip) {
            Some(height) => height + 1,
            None => 1,
        };
        for (key, value_hash) in self.pending_hashes.drain(..) {
            let (target_tip, target_height) = if cur_tip == &BOOT_BLOCK_ID {
                (cur_tip.clone(), next_height - 1)
            } else {
                (next_tip.clone(), next_height)
            };

            wrb_test_debug!(
                "Dump '{}' = '{}' at {},{} (data = {:?})",
                &key,
                &value_hash,
                &target_tip,
                target_height,
                self.pending_data.get(&value_hash)
            );
            let values: &[&dyn ToSql] =
                &[&target_tip, &u64_to_sql(target_height)?, &key, &value_hash];
            conn.execute(
                "REPLACE INTO kvstore (chain_tip, height, key, data_hash) VALUES (?1, ?2, ?3, ?4)",
                values,
            )
            .map_err(|e| Error::DBError(db_error::SqliteError(e)))?;

            let value = self
                .pending_data
                .get(&value_hash)
                .cloned()
                .expect("BUG: have hash but no value");
            SqliteConnection::put(conn, &value_hash, &value)?;
        }
        self.pending_hashes.clear();
        self.pending_data.clear();
        self.pending_index.clear();
        Ok(())
    }
}

impl<'a> ReadOnlyWrbStore<'a> {
    pub fn as_clarity_db<'b>(
        &'b mut self,
        headers_db: &'b dyn HeadersDB,
        burn_state_db: &'b dyn BurnStateDB,
    ) -> ClarityDatabase<'b> {
        ClarityDatabase::new(self, headers_db, burn_state_db)
    }

    pub fn as_analysis_db<'b>(&'b mut self) -> AnalysisDatabase<'b> {
        AnalysisDatabase::new(self)
    }

    pub fn mainnet(&self) -> bool {
        self.mainnet
    }
}

impl<'a> ClarityBackingStore for ReadOnlyWrbStore<'a> {
    fn get_side_store(&mut self) -> &Connection {
        &self.conn
    }

    fn get_cc_special_cases_handler(&self) -> Option<SpecialCaseHandler> {
        Some(&handle_wrb_contract_call_special_cases)
    }

    fn set_block_hash(&mut self, bhh: StacksBlockId) -> Result<StacksBlockId, ClarityError> {
        if !check_block_hash(self.conn, self.tip_height, &bhh).expect("FATAL: failed to query DB") {
            return Err(RuntimeErrorType::UnknownBlockHeaderHash(BlockHeaderHash(bhh.0)).into());
        }
        let new_tip_height = tipless_get_block_height_of(self.conn, &bhh)
            .expect("FATAL: failed to query height from DB")
            .ok_or(RuntimeErrorType::UnknownBlockHeaderHash(BlockHeaderHash(
                bhh.0,
            )))?;

        let result = Ok(self.chain_tip);

        self.chain_tip = bhh;
        self.tip_height = new_tip_height;

        result
    }

    fn get_current_block_height(&mut self) -> u32 {
        match get_block_height_of(self.conn, self.tip_height, &self.chain_tip) {
            Ok(Some(x)) => x.try_into().expect("FATAL: block height too high"),
            Ok(None) => {
                if self.chain_tip == BOOT_BLOCK_ID {
                    // the current block height should always work, except if it's the first block
                    // height (in which case, the current chain tip should match the first-ever
                    // index block hash).
                    return 0;
                }

                // should never happen
                let msg = format!(
                    "Failed to obtain current block height of {} (got None)",
                    &self.chain_tip
                );
                wrb_error!("{}", &msg);
                panic!("{}", &msg);
            }
            Err(e) => {
                let msg = format!(
                    "Unexpected K/V failure: Failed to get current block height of {}: {:?}",
                    &self.chain_tip, &e
                );
                wrb_error!("{}", &msg);
                panic!("{}", &msg);
            }
        }
    }

    fn get_block_at_height(&mut self, block_height: u32) -> Option<StacksBlockId> {
        let bhh = get_block_at_height(&self.conn, self.tip_height, block_height.into())
            .expect(&format!("FATAL: no block at height {}", block_height));
        wrb_debug!(
            "bhh at height {} for {} = {:?}",
            block_height,
            self.tip_height,
            &bhh
        );
        bhh
    }

    fn get_open_chain_tip(&mut self) -> StacksBlockId {
        self.chain_tip.clone()
    }

    fn get_open_chain_tip_height(&mut self) -> u32 {
        wrb_debug!(
            "Open chain tip is {} (tip is {})",
            self.tip_height,
            &self.chain_tip
        );
        self.tip_height.try_into().expect("Block height too high")
    }

    fn get_data(&mut self, key: &str) -> Result<Option<String>, clarity_error> {
        wrb_test_debug!("Get hash for '{}' at height {}", key, self.tip_height);
        let Some(hash) = get_hash(self.conn, self.tip_height, key).map_err(|e| {
            clarity_error::Interpreter(clarity::vm::errors::InterpreterError::Expect(format!(
                "failed to get hash: {:?}",
                &e
            )))
        })?
        else {
            return Ok(None);
        };
        wrb_test_debug!(
            "Hash for '{}' at height {} is '{}'",
            key,
            self.tip_height,
            &hash
        );
        let value_opt = SqliteConnection::get(self.get_side_store(), &hash).expect(&format!(
            "FATAL: kvstore contained value hash not found in side storage: {}",
            &hash
        ));

        return Ok(value_opt);
    }

    fn get_data_with_proof(&mut self, _: &str) -> Result<Option<(String, Vec<u8>)>, clarity_error> {
        unimplemented!()
    }

    fn get_data_from_path(&mut self, _path: &TrieHash) -> Result<Option<String>, clarity_error> {
        unimplemented!()
    }

    fn get_data_with_proof_from_path(
        &mut self,
        _path: &TrieHash,
    ) -> Result<Option<(String, Vec<u8>)>, clarity_error> {
        unimplemented!()
    }

    fn put_all_data(&mut self, _items: Vec<(String, String)>) -> Result<(), clarity_error> {
        wrb_error!("Attempted to commit changes to read-only K/V");
        panic!("BUG: attempted commit to read-only K/V");
    }

    fn get_contract_hash(
        &mut self,
        contract_id: &QualifiedContractIdentifier,
    ) -> Result<(StacksBlockId, Sha512Trunc256Sum), clarity_error> {
        sqlite_get_contract_hash(self, contract_id)
    }

    fn insert_metadata(
        &mut self,
        _contract_id: &QualifiedContractIdentifier,
        _key: &str,
        _value: &str,
    ) -> Result<(), clarity_error> {
        wrb_error!("Attempted to write metadata to read-only K/V");
        panic!("BUG: attempted to write metadata to read-only K/V");
    }

    fn get_metadata(
        &mut self,
        contract_id: &QualifiedContractIdentifier,
        key: &str,
    ) -> Result<Option<String>, clarity_error> {
        sqlite_get_metadata(self, contract_id, key)
    }

    fn get_metadata_manual(
        &mut self,
        at_height: u32,
        contract_id: &QualifiedContractIdentifier,
        key: &str,
    ) -> Result<Option<String>, clarity_error> {
        sqlite_get_metadata_manual(self, at_height, contract_id, key)
    }
}

impl<'a> WritableWrbStore<'a> {
    pub fn as_clarity_db<'b>(
        &'b mut self,
        headers_db: &'b dyn HeadersDB,
        burn_state_db: &'b dyn BurnStateDB,
    ) -> ClarityDatabase<'b> {
        ClarityDatabase::new(self, headers_db, burn_state_db)
    }

    pub fn as_analysis_db<'b>(&'b mut self) -> AnalysisDatabase<'b> {
        AnalysisDatabase::new(self)
    }

    pub fn commit_to(mut self, final_bhh: &StacksBlockId) -> Result<(), Error> {
        let target_tip = if self.chain_tip == BOOT_BLOCK_ID {
            &self.chain_tip
        } else {
            &self.next_tip
        };

        wrb_test_debug!("commit_to({} --> {})", target_tip, final_bhh);
        SqliteConnection::commit_metadata_to(&self.tx, target_tip, final_bhh)?;
        self.write_buf.dump(&self.tx, target_tip, final_bhh)?;

        let args: &[&dyn ToSql] = &[final_bhh, target_tip];
        self.tx.execute(
            "UPDATE kvstore SET chain_tip = ?1 WHERE chain_tip = ?2",
            args,
        )?;

        self.tx.commit()?;
        Ok(())
    }

    pub fn commit(self) -> Result<(), Error> {
        let final_bhh = self.next_tip.clone();
        self.commit_to(&final_bhh)
    }

    pub fn rollback_block(self) {
        // no-op for now
    }

    pub fn mainnet(&self) -> bool {
        self.mainnet
    }
}

impl<'a> ClarityBackingStore for WritableWrbStore<'a> {
    fn set_block_hash(&mut self, bhh: StacksBlockId) -> Result<StacksBlockId, ClarityError> {
        if !check_block_hash(&self.tx, self.tip_height, &bhh)
            .expect("FATAL: failed to query hash from DB")
        {
            return Err(RuntimeErrorType::UnknownBlockHeaderHash(BlockHeaderHash(bhh.0)).into());
        }
        let new_tip_height = tipless_get_block_height_of(&self.tx, &bhh)
            .expect("FATAL: failed to query height for hash in DB")
            .ok_or(ClarityError::Runtime(
                RuntimeErrorType::UnknownBlockHeaderHash(BlockHeaderHash(bhh.0)),
                None,
            ))?;

        let result = Ok(self.chain_tip);

        self.chain_tip = bhh;
        self.tip_height = new_tip_height;

        result
    }

    fn get_cc_special_cases_handler(&self) -> Option<SpecialCaseHandler> {
        Some(&handle_wrb_contract_call_special_cases)
    }

    fn get_data(&mut self, key: &str) -> Result<Option<String>, clarity_error> {
        if let Some(ref value) = self.write_buf.get(key) {
            return Ok(Some(value.clone()));
        }
        wrb_test_debug!("Get hash for '{}' at height {}", key, self.tip_height);
        let Some(hash) = get_hash(&self.tx, self.tip_height, key).map_err(|e| {
            clarity_error::Interpreter(clarity::vm::errors::InterpreterError::Expect(format!(
                "failed to get hash: {:?}",
                &e
            )))
        })?
        else {
            return Ok(None);
        };
        wrb_test_debug!(
            "Hash for '{}' at height {} is '{}'",
            key,
            self.tip_height,
            &hash
        );
        let value_opt = SqliteConnection::get(self.get_side_store(), &hash).expect(&format!(
            "FATAL: kvstore contained value hash not found in side storage: {}",
            &hash
        ));

        return Ok(value_opt);
    }

    fn get_data_with_proof(
        &mut self,
        _key: &str,
    ) -> Result<Option<(String, Vec<u8>)>, clarity_error> {
        unimplemented!()
    }

    fn get_data_from_path(&mut self, _path: &TrieHash) -> Result<Option<String>, clarity_error> {
        unimplemented!()
    }

    fn get_data_with_proof_from_path(
        &mut self,
        _path: &TrieHash,
    ) -> Result<Option<(String, Vec<u8>)>, clarity_error> {
        unimplemented!()
    }

    fn get_side_store(&mut self) -> &Connection {
        &self.tx
    }

    fn get_block_at_height(&mut self, block_height: u32) -> Option<StacksBlockId> {
        let bhh = get_block_at_height(&self.tx, self.tip_height, block_height.into())
            .expect(&format!("FATAL: no block at height {}", block_height));
        bhh
    }

    fn get_open_chain_tip(&mut self) -> StacksBlockId {
        if self.chain_tip == BOOT_BLOCK_ID {
            self.chain_tip.clone()
        } else {
            self.next_tip.clone()
        }
    }

    fn get_open_chain_tip_height(&mut self) -> u32 {
        self.tip_height
            .try_into()
            .expect("FATAL: block height too big")
    }

    fn get_current_block_height(&mut self) -> u32 {
        match get_block_height_of(&self.tx, self.tip_height, &self.chain_tip) {
            Ok(Some(x)) => x.try_into().expect("FATAL: block height too big"),
            Ok(None) => {
                if self.chain_tip == BOOT_BLOCK_ID {
                    // the current block height should always work, except if it's the first block
                    // height (in which case, the current chain tip should match the first-ever
                    // index block hash).
                    return 0;
                }

                // should never happen
                let msg = format!(
                    "Failed to obtain current block height of {} (got None)",
                    &self.chain_tip
                );
                wrb_error!("{}", &msg);
                panic!("{}", &msg);
            }
            Err(e) => {
                let msg = format!(
                    "Unexpected K/V failure: Failed to get current block height of {}: {:?}",
                    &self.chain_tip, &e
                );
                wrb_error!("{}", &msg);
                panic!("{}", &msg);
            }
        }
    }

    fn put_all_data(&mut self, items: Vec<(String, String)>) -> Result<(), clarity_error> {
        for (key, value) in items.into_iter() {
            self.write_buf.put(&key, &value);
        }
        self.write_buf
            .dump(&self.tx, &self.chain_tip, &self.next_tip)
            .expect("FATAL: DB error");

        Ok(())
    }

    fn get_contract_hash(
        &mut self,
        contract_id: &QualifiedContractIdentifier,
    ) -> Result<(StacksBlockId, Sha512Trunc256Sum), clarity_error> {
        sqlite_get_contract_hash(self, contract_id)
    }

    fn insert_metadata(
        &mut self,
        contract_id: &QualifiedContractIdentifier,
        key: &str,
        value: &str,
    ) -> Result<(), clarity_error> {
        sqlite_insert_metadata(self, contract_id, key, value)
    }

    fn get_metadata(
        &mut self,
        contract_id: &QualifiedContractIdentifier,
        key: &str,
    ) -> Result<Option<String>, clarity_error> {
        sqlite_get_metadata(self, contract_id, key)
    }

    fn get_metadata_manual(
        &mut self,
        at_height: u32,
        contract_id: &QualifiedContractIdentifier,
        key: &str,
    ) -> Result<Option<String>, clarity_error> {
        sqlite_get_metadata_manual(self, at_height, contract_id, key)
    }
}
