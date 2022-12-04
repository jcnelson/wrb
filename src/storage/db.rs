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

use std::path::PathBuf;
use std::collections::{HashMap};

use rand::Rng;

use rusqlite::Connection;
use rusqlite::Transaction;
use rusqlite::OpenFlags;
use rusqlite::NO_PARAMS;
use rusqlite::ToSql;

use clarity::vm::analysis::AnalysisDatabase;
use clarity::vm::database::{
    BurnStateDB, ClarityDatabase, HeadersDB, SqliteConnection,
};
use clarity::vm::errors::{
    RuntimeErrorType, Error as ClarityError
};

use clarity::vm::database::SpecialCaseHandler;
use clarity::vm::database::ClarityBackingStore;
use stacks_common::types::chainstate::BlockHeaderHash;
use stacks_common::types::chainstate::{StacksBlockId};
use stacks_common::util::hash::Sha512Trunc256Sum;

use crate::storage::Error;
use crate::storage::WrbDB;
use crate::storage::WrbHeadersDB;
use crate::storage::ReadOnlyWrbStore;
use crate::storage::WritableWrbStore;
use crate::storage::WriteBuffer;
use crate::storage::util::*;
use crate::util::sqlite::{sqlite_open, tx_begin_immediate, query_row, u64_to_sql, Error as db_error};

use crate::vm::{GENESIS_BLOCK_ID, BOOT_BLOCK_ID};

const SCHEMA_VERSION : &'static str = "1";

const KV_SCHEMA: &'static [&'static str] = &[
    r#"
    CREATE TABLE IF NOT EXISTS kvstore(
        chain_tip TEXT NOT NULL,
        height INTEGER NOT NULL,
        key TEXT NOT NULL,
        data_hash TEXT NOT NULL,
        PRIMARY KEY(chain_tip, height, key, data_hash)
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
fn tipless_get_block_height_of(conn: &Connection, bhh: &StacksBlockId) -> Result<Option<u64>, Error> {
    if bhh == &GENESIS_BLOCK_ID || bhh == &BOOT_BLOCK_ID {
        return Ok(Some(0))
    }
    query_row::<u64, _>(conn, "SELECT height FROM kvstore WHERE chain_tip = ?1", &[bhh as &dyn ToSql])
        .map_err(|e| e.into())
}

/// Check to see if the block hash exists in the K/V
fn check_block_hash(conn: &Connection, tip_height: u64, bhh: &StacksBlockId) -> Result<bool, Error> {
    match query_row::<i64, _>(conn, "SELECT 1 FROM kvstore WHERE chain_tip = ?1 AND height <= ?2", &[bhh as &dyn ToSql, &u64_to_sql(tip_height)?]) {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(e) => Err(e.into())
    }
}

/// Get the height of a block hash
fn get_block_height_of(conn: &Connection, tip_height: u64, bhh: &StacksBlockId) -> Result<Option<u64>, Error> {
    query_row::<u64, _>(conn, "SELECT height FROM kvstore WHERE chain_tip = ?1 AND height <= ?2", &[bhh as &dyn ToSql, &u64_to_sql(tip_height)?])
        .map_err(|e| e.into())
}

/// Get the hash of a block given a height
fn get_block_at_height(conn: &Connection, tip_height: u64, height: u64) -> Result<Option<StacksBlockId>, Error> {
    if tip_height < height {
        return Ok(None)
    }
    query_row::<StacksBlockId, _>(conn, "SELECT chain_tip FROM kvstore WHERE height = ?1", &[&u64_to_sql(tip_height)?])
        .map_err(|e| e.into())
}

/// Get the hash of a value given the block hash and tip height
fn get_hash(conn: &Connection, tip_height: u64, key: &str) -> Result<Option<String>, Error> {
    let args : &[&dyn ToSql] = &[&key.to_string(), &u64_to_sql(tip_height)?];
    query_row::<String, _>(conn, "SELECT data_hash FROM kvstore WHERE key = ?1 AND height = ?2", args)
        .map_err(|e| e.into())
}

/// Get the highest height stored
fn tipless_get_highest_tip_height(conn: &Connection) -> Result<u64, Error> {
    query_row::<u64, _>(conn, "SELECT IFNULL(MAX(height),0) FROM kvstore", NO_PARAMS)
        .map_err(|e| e.into())
        .and_then(|height_opt| {
            Ok(height_opt.unwrap_or(0))
        })
}

impl WrbHeadersDB {
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

impl WrbDB {
    fn setup_db(
        path_str: &str,
        domain: &str
    ) -> Result<Connection, Error> {
        let mut path = PathBuf::from(path_str);
        path.push(domain);

        std::fs::create_dir_all(&path)?;

        path.push("db.sqlite");
        let open_flags = if std::fs::metadata(&path).is_ok() {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        }
        else {
            OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_READ_WRITE
        };

        let mut conn = sqlite_open(&path, open_flags, true)?;

        if SqliteConnection::check_schema(&conn).is_ok() {
            // no need to initialize
            return Ok(conn);
        }

        let tx = tx_begin_immediate(&mut conn)?;

        SqliteConnection::initialize_conn(&tx)
            .map_err(|e| {
                error!("Failed to initialize DB: {:?}", &e);
                Error::InitializationFailure
            })?;

        for cmd in KV_SCHEMA.iter() {
            tx.execute(cmd, NO_PARAMS)?;
        }

        tx.commit()?;
        Ok(conn)
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
        let conn = WrbDB::setup_db(path_str, domain)?;
        let chain_tip = match chain_tip {
            Some(ref tip) => (*tip).clone(),
            None => BOOT_BLOCK_ID.clone()
        };

        Ok(WrbDB {
            db_path: path_str.to_string(),
            domain: domain.to_string(),
            conn,
            chain_tip
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

    /// Create a headers DB for this DB
    pub fn headers_db(&self) -> WrbHeadersDB {
        let conn = Self::open_readonly(&self.db_path, &self.domain)
            .expect("FATAL: could not open read only");

        WrbHeadersDB {
            conn
        }
    }

    /// Begin a read-only session at a particular point in time.
    pub fn begin_read_only<'a>(
        &'a self,
        at_block: Option<&StacksBlockId>,
    ) -> ReadOnlyWrbStore<'a> {
        let chain_tip = at_block.map(|b| (*b).clone()).unwrap_or(self.chain_tip.clone());
        let tip_height = tipless_get_block_height_of(&self.conn, &chain_tip)
            .expect("FATAL: DB error")
            .expect(&format!("FATAL: do not have height for tip {}", &chain_tip));

        ReadOnlyWrbStore {
            chain_tip,
            tip_height,
            conn: &self.conn
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
        let tx = tx_begin_immediate(&mut self.conn)
            .expect("FATAL: could not begin transaction");

        let tip_height = tipless_get_block_height_of(&tx, current)
            .expect("DB failure")
            .expect(&format!("FATAL: do not have height for tip {}", current));

        let current_tip_height = 
            if current == &BOOT_BLOCK_ID {
                0
            }
            else {
                get_block_height_of(&tx, tip_height, current)
                    .expect("DB failure")
                    .expect(&format!("FATAL: given tip {} has no known block height as of tip {} (height {})", current, current, tip_height))
            };

        WritableWrbStore {
            chain_tip: current.clone(),
            tip_height: current_tip_height,
            next_tip: next.clone(),
            write_buf: WriteBuffer::new(),
            tx: tx
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
            pending_index: HashMap::new()
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
        }
        else {
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

    pub fn dump(&mut self, conn: &Connection, cur_tip: &StacksBlockId, next_tip: &StacksBlockId) -> Result<(), Error> {
        let next_height = match get_wrb_block_height(conn, cur_tip) {
            Some(height) => height + 1,
            None => 0
        };
        for (key, value_hash) in self.pending_hashes.drain(..) {
            test_debug!("Dump '{}' = '{}' at {},{}", &key, &value_hash, &next_tip, next_height);
            let values: &[&dyn ToSql] = &[&next_tip, &u64_to_sql(next_height)?, &key, &value_hash];
            conn.execute("REPLACE INTO kvstore (chain_tip, height, key, data_hash) VALUES (?1, ?2, ?3, ?4)", values)
                .map_err(|e| Error::DBError(db_error::SqliteError(e)))?;

            let value = self.pending_data.get(&value_hash).cloned().expect("BUG: have hash but no value");
            SqliteConnection::put(conn, &value_hash, &value);
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
}

impl<'a> ClarityBackingStore for ReadOnlyWrbStore<'a> {
    fn get_side_store(&mut self) -> &Connection {
        &self.conn
    }

    fn get_cc_special_cases_handler(&self) -> Option<SpecialCaseHandler> {
        None
    }

    fn set_block_hash(&mut self, bhh: StacksBlockId) -> Result<StacksBlockId, ClarityError> {
        if !check_block_hash(self.conn, self.tip_height, &bhh).expect("FATAL: failed to query DB") {
            return Err(RuntimeErrorType::UnknownBlockHeaderHash(BlockHeaderHash(bhh.0)).into());
        }
        let new_tip_height = tipless_get_block_height_of(self.conn, &bhh)
            .expect("FATAL: failed to query height from DB")
            .ok_or(RuntimeErrorType::UnknownBlockHeaderHash(BlockHeaderHash(bhh.0)))?;

        let result = Ok(self.chain_tip);

        self.chain_tip = bhh;
        self.tip_height = new_tip_height;

        result
    }

    fn get_current_block_height(&mut self) -> u32 {
        match get_block_height_of(self.conn, self.tip_height, &self.chain_tip) {
            Ok(Some(x)) => x.try_into().expect("FATAL: block height too high"),
            Ok(None) => {
                if self.chain_tip == GENESIS_BLOCK_ID {
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
                error!("{}", &msg);
                panic!("{}", &msg);
            }
            Err(e) => {
                let msg = format!(
                    "Unexpected K/V failure: Failed to get current block height of {}: {:?}",
                    &self.chain_tip, &e
                );
                error!("{}", &msg);
                panic!("{}", &msg);
            }
        }
    }

    fn get_block_at_height(&mut self, block_height: u32) -> Option<StacksBlockId> {
        get_block_at_height(&self.conn, self.tip_height, block_height.into())
            .expect(&format!("FATAL: no block at height {}", block_height))
    }

    fn get_open_chain_tip(&mut self) -> StacksBlockId {
        self.chain_tip.clone()
    }

    fn get_open_chain_tip_height(&mut self) -> u32 {
        self.tip_height.try_into().expect("Block height too high")
    }

    fn get(&mut self, key: &str) -> Option<String> {
        trace!("ClarityKV get: {:?} tip={}", key, &self.chain_tip);
        let hash = get_hash(self.conn, self.tip_height, key)
            .expect("FATAL: kvstore read error")?;
        let value = SqliteConnection::get(self.get_side_store(), &hash)
            .expect(&format!("FATAL: kvstore contained value hash not found in side storage: {}", &hash));

        Some(value)
    }

    fn get_with_proof(&mut self, _: &str) -> Option<(String, Vec<u8>)> {
        unimplemented!()
    }

    fn put_all(&mut self, _items: Vec<(String, String)>) {
        error!("Attempted to commit changes to read-only K/V");
        panic!("BUG: attempted commit to read-only K/V");
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

    pub fn commit_to(mut self, final_bhh: &StacksBlockId) {
        debug!("commit_to({})", final_bhh);
        SqliteConnection::commit_metadata_to(&self.tx, &self.chain_tip, final_bhh);
        self.write_buf.dump(&self.tx, &self.chain_tip, final_bhh)
            .expect("FATAL: failed to store written buffer to the DB");

        self.tx.commit().expect("FATAL: failed to commit changes");
    }

    pub fn commit(self) {
        let final_bhh = self.next_tip.clone();
        self.commit_to(&final_bhh)
    }

    pub fn rollback_block(self) {
        // no-op for now
    }
}

impl<'a> ClarityBackingStore for WritableWrbStore<'a> {
    fn set_block_hash(&mut self, bhh: StacksBlockId) -> Result<StacksBlockId, ClarityError> {
        if !check_block_hash(&self.tx, self.tip_height, &bhh).expect("FATAL: failed to query hash from DB") {
            return Err(RuntimeErrorType::UnknownBlockHeaderHash(BlockHeaderHash(bhh.0)).into())
        }
        let new_tip_height = tipless_get_block_height_of(&self.tx, &bhh)
            .expect("FATAL: failed to query height for hash in DB")
            .ok_or(ClarityError::Runtime(RuntimeErrorType::UnknownBlockHeaderHash(BlockHeaderHash(bhh.0)), None))?;

        let result = Ok(self.chain_tip);

        self.chain_tip = bhh;
        self.tip_height = new_tip_height;

        result
    }

    fn get_cc_special_cases_handler(&self) -> Option<SpecialCaseHandler> {
        None
    }

    fn get(&mut self, key: &str) -> Option<String> {
        if let Some(ref value) = self.write_buf.get(key) {
            Some(value.clone())
        }
        else if let Some(ref value_hash) = get_hash(&self.tx, self.tip_height, key).expect("FATAL: failed to query hash from DB") {
            Some(SqliteConnection::get(&self.tx, value_hash).expect(&format!(
                "ERROR: K/V contained value_hash not found in side storage: {}",
                value_hash
            )))
        }
        else {
            None
        }
    }

    fn get_with_proof(&mut self, _key: &str) -> Option<(String, Vec<u8>)> {
        unimplemented!()
    }

    fn get_side_store(&mut self) -> &Connection {
        &self.tx
    }

    fn get_block_at_height(&mut self, height: u32) -> Option<StacksBlockId> {
        get_block_at_height(&self.tx, self.tip_height, height.into())
            .expect("FATAL: failed to query block at height from DB")
    }

    fn get_open_chain_tip(&mut self) -> StacksBlockId {
        self.chain_tip.clone()
    }

    fn get_open_chain_tip_height(&mut self) -> u32 {
        self.tip_height.try_into().expect("FATAL: block height too big")
    }

    fn get_current_block_height(&mut self) -> u32 {
        match get_block_height_of(&self.tx, self.tip_height, &self.chain_tip) {
            Ok(Some(x)) => x.try_into().expect("FATAL: block height too big"),
            Ok(None) => {
                if self.chain_tip == GENESIS_BLOCK_ID {
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
                error!("{}", &msg);
                panic!("{}", &msg);
            }
            Err(e) => {
                let msg = format!(
                    "Unexpected K/V failure: Failed to get current block height of {}: {:?}",
                    &self.chain_tip, &e
                );
                error!("{}", &msg);
                panic!("{}", &msg);
            }
        }
    }

    fn put_all(&mut self, items: Vec<(String, String)>) {
        for (key, value) in items.into_iter() {
            self.write_buf.put(&key, &value);
        }
        self.write_buf.dump(&self.tx, &self.chain_tip, &self.next_tip)
            .expect("FATAL: DB error");
    }
}

