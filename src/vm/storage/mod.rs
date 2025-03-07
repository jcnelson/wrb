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

use crate::util::sqlite::Error as db_error;
use std::error;
use std::fmt;
use std::io;
use std::io::Error as io_error;

use rusqlite::Connection;
use rusqlite::Error as sqlite_error;
use rusqlite::Transaction;

use stacks_common::types::chainstate::StacksBlockId;
use stacks_common::util::hash::to_hex;

use clarity::vm::errors::Error as clarity_error;

pub mod db;
pub mod headers;
pub mod util;

#[derive(Debug)]
pub enum Error {
    UnknownBlockHeaderHash([u8; 32]),
    DBError(db_error),
    InitializationFailure,
    Clarity(String),
}

impl From<db_error> for Error {
    fn from(e: db_error) -> Error {
        Error::DBError(e)
    }
}

impl From<io_error> for Error {
    fn from(e: io_error) -> Error {
        Error::DBError(db_error::IOError(e))
    }
}

impl From<sqlite_error> for Error {
    fn from(e: sqlite_error) -> Error {
        Error::DBError(db_error::SqliteError(e))
    }
}

impl From<clarity_error> for Error {
    fn from(e: clarity_error) -> Error {
        Error::Clarity(format!("{:?}", &e))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::UnknownBlockHeaderHash(ref hash_bytes) => {
                write!(f, "Unknown block header hash {}", &to_hex(hash_bytes))
            }
            Error::DBError(ref e) => e.fmt(f),
            Error::InitializationFailure => write!(f, "Initialization failure"),
            Error::Clarity(ref e) => e.fmt(f),
        }
    }
}

impl error::Error for Error {
    fn cause(&self) -> Option<&dyn error::Error> {
        match *self {
            Error::UnknownBlockHeaderHash(ref _hash) => None,
            Error::DBError(ref db_error) => Some(db_error),
            Error::InitializationFailure => None,
            Error::Clarity(..) => None,
        }
    }
}

/// Internal data structure for data we're pending to commit
struct WriteBuffer {
    /// Sequence of writes we'll store on commit
    pending_hashes: Vec<(String, String)>,
    pending_data: HashMap<String, String>,
    pending_index: HashMap<String, usize>,
}

/// Outermost DB implementation for wrb.
/// Namespaced by domain
pub struct WrbDB {
    db_path: String,
    domain: String,
    chain_tip: StacksBlockId,
    conn: Connection,
    created: bool,
}

pub struct WritableWrbStore<'a> {
    /// Block ID at which this KV was opened
    chain_tip: StacksBlockId,
    /// Height of `chain_tip`
    tip_height: u64,
    /// Block ID of the chain tip we're building
    next_tip: StacksBlockId,
    /// Open TX to the underlying store
    tx: Transaction<'a>,
    /// Pending writes
    write_buf: WriteBuffer,
}

pub struct ReadOnlyWrbStore<'a> {
    /// Block ID at which this KV was opened
    chain_tip: StacksBlockId,
    /// Height of `chain_tip`
    tip_height: u64,
    /// Connection to the underlying store
    conn: &'a Connection,
}

pub struct WrbHeadersDB {
    /// Owned read-only connection to the underlying datastore
    conn: Connection,
}
