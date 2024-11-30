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
use std::fmt;
use std::io::Error as IOError;
use std::path::Path;

use stacks_common::util::sleep_ms;

use stacks_common::types::chainstate::StacksBlockId;

use clarity::vm::types::QualifiedContractIdentifier;

use stacks_common::util::secp256k1::Secp256k1PrivateKey;
use stacks_common::util::secp256k1::Secp256k1PublicKey;

use rusqlite::types::ToSql;
use rusqlite::Connection;
use rusqlite::Error as sqlite_error;
use rusqlite::OpenFlags;
use rusqlite::OptionalExtension;
use rusqlite::Row;
use rusqlite::Transaction;
use rusqlite::TransactionBehavior;

use rand::thread_rng;
use rand::Rng;

use serde_json::Error as serde_error;

pub type DBConn = rusqlite::Connection;
pub type DBTx<'a> = rusqlite::Transaction<'a>;

// 256MB
pub const SQLITE_MMAP_SIZE: i64 = 256 * 1024 * 1024;

#[derive(Debug)]
pub enum Error {
    /// Not implemented
    NotImplemented,
    /// Database doesn't exist
    NoDBError,
    /// Read-only and tried to write
    ReadOnly,
    /// Type error -- can't represent the given data in the database
    TypeError,
    /// Database is corrupt -- we got data that shouldn't be there, or didn't get data when we
    /// should have
    Corruption,
    /// Serialization error -- can't serialize data
    SerializationError(serde_error),
    /// Parse error -- failed to load data we stored directly
    ParseError,
    /// Operation would overflow
    Overflow,
    /// Data not found
    NotFoundError,
    /// Data already exists
    ExistsError,
    /// Sqlite3 error
    SqliteError(sqlite_error),
    /// I/O error
    IOError(IOError),
    /// Old schema error
    OldSchema(u64),
    /// Database is too old for epoch
    TooOldForEpoch,
    /// Other error
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::NotImplemented => write!(f, "Not implemented"),
            Error::NoDBError => write!(f, "Database does not exist"),
            Error::ReadOnly => write!(f, "Database is opened read-only"),
            Error::TypeError => write!(f, "Invalid or unrepresentable database type"),
            Error::Corruption => write!(f, "Database is corrupt"),
            Error::SerializationError(ref e) => fmt::Display::fmt(e, f),
            Error::ParseError => write!(f, "Parse error"),
            Error::Overflow => write!(f, "Numeric overflow"),
            Error::NotFoundError => write!(f, "Not found"),
            Error::ExistsError => write!(f, "Already exists"),
            Error::IOError(ref e) => fmt::Display::fmt(e, f),
            Error::SqliteError(ref e) => fmt::Display::fmt(e, f),
            Error::OldSchema(ref s) => write!(f, "Old database schema: {}", s),
            Error::TooOldForEpoch => {
                write!(f, "Database is not compatible with current system epoch")
            }
            Error::Other(ref s) => fmt::Display::fmt(s, f),
        }
    }
}

impl error::Error for Error {
    fn cause(&self) -> Option<&dyn error::Error> {
        match *self {
            Error::NotImplemented => None,
            Error::NoDBError => None,
            Error::ReadOnly => None,
            Error::TypeError => None,
            Error::Corruption => None,
            Error::SerializationError(ref e) => Some(e),
            Error::ParseError => None,
            Error::Overflow => None,
            Error::NotFoundError => None,
            Error::ExistsError => None,
            Error::SqliteError(ref e) => Some(e),
            Error::IOError(ref e) => Some(e),
            Error::OldSchema(ref _s) => None,
            Error::TooOldForEpoch => None,
            Error::Other(ref _s) => None,
        }
    }
}

impl From<sqlite_error> for Error {
    fn from(e: sqlite_error) -> Error {
        Error::SqliteError(e)
    }
}

pub trait FromRow<T> {
    fn from_row<'a>(row: &'a Row) -> Result<T, Error>;
}

pub trait FromColumn<T> {
    fn from_column<'a>(row: &'a Row, column_name: &str) -> Result<T, Error>;
}

impl FromRow<u64> for u64 {
    fn from_row<'a>(row: &'a Row) -> Result<u64, Error> {
        let x: i64 = row.get_unwrap(0);
        if x < 0 {
            return Err(Error::ParseError);
        }
        Ok(x as u64)
    }
}

impl FromRow<String> for String {
    fn from_row<'a>(row: &'a Row) -> Result<String, Error> {
        let x: String = row.get_unwrap(0);
        Ok(x)
    }
}

impl FromColumn<u64> for u64 {
    fn from_column<'a>(row: &'a Row, column_name: &str) -> Result<u64, Error> {
        let x: i64 = row.get_unwrap(column_name);
        if x < 0 {
            return Err(Error::ParseError);
        }
        Ok(x as u64)
    }
}

impl FromColumn<Option<u64>> for u64 {
    fn from_column<'a>(row: &'a Row, column_name: &str) -> Result<Option<u64>, Error> {
        let x: Option<i64> = row.get_unwrap(column_name);
        match x {
            Some(x) => {
                if x < 0 {
                    return Err(Error::ParseError);
                }
                Ok(Some(x as u64))
            }
            None => Ok(None),
        }
    }
}

impl FromRow<i64> for i64 {
    fn from_row<'a>(row: &'a Row) -> Result<i64, Error> {
        let x: i64 = row.get_unwrap(0);
        Ok(x)
    }
}

impl FromColumn<i64> for i64 {
    fn from_column<'a>(row: &'a Row, column_name: &str) -> Result<i64, Error> {
        let x: i64 = row.get_unwrap(column_name);
        Ok(x)
    }
}

impl FromRow<StacksBlockId> for StacksBlockId {
    fn from_row<'a>(row: &'a Row) -> Result<StacksBlockId, Error> {
        let x: String = row.get_unwrap(0);
        StacksBlockId::from_hex(&x).map_err(|_| Error::ParseError)
    }
}

impl FromColumn<QualifiedContractIdentifier> for QualifiedContractIdentifier {
    fn from_column<'a>(
        row: &'a Row,
        column_name: &str,
    ) -> Result<QualifiedContractIdentifier, Error> {
        let value: String = row.get_unwrap(column_name);
        QualifiedContractIdentifier::parse(&value).map_err(|_| Error::ParseError)
    }
}

/// Make public keys loadable from a sqlite database
impl FromColumn<Secp256k1PublicKey> for Secp256k1PublicKey {
    fn from_column<'a>(row: &'a Row, column_name: &str) -> Result<Secp256k1PublicKey, Error> {
        let pubkey_hex: String = row.get_unwrap(column_name);
        let pubkey = Secp256k1PublicKey::from_hex(&pubkey_hex).map_err(|_e| Error::ParseError)?;
        Ok(pubkey)
    }
}

/// Make private keys loadable from a sqlite database
impl FromColumn<Secp256k1PrivateKey> for Secp256k1PrivateKey {
    fn from_column<'a>(row: &'a Row, column_name: &str) -> Result<Secp256k1PrivateKey, Error> {
        let privkey_hex: String = row.get_unwrap(column_name);
        let privkey =
            Secp256k1PrivateKey::from_hex(&privkey_hex).map_err(|_e| Error::ParseError)?;
        Ok(privkey)
    }
}

pub fn u64_to_sql(x: u64) -> Result<i64, Error> {
    if x > (i64::MAX as u64) {
        return Err(Error::ParseError);
    }
    Ok(x as i64)
}

macro_rules! impl_byte_array_from_column_only {
    ($thing:ident) => {
        impl crate::util::sqlite::FromColumn<$thing> for $thing {
            fn from_column(
                row: &rusqlite::Row,
                column_name: &str,
            ) -> Result<Self, crate::util::sqlite::Error> {
                Ok(row.get_unwrap::<_, Self>(column_name))
            }
        }
    };
}

impl_byte_array_from_column_only!(StacksBlockId);

/// Load the path of the database from the connection
#[cfg(test)]
fn get_db_path(conn: &Connection) -> Result<String, Error> {
    let sql = "PRAGMA database_list";
    let path: Result<Option<String>, sqlite_error> =
        conn.query_row_and_then(sql, rusqlite::params![], |row| row.get(2));
    match path {
        Ok(Some(path)) => Ok(path),
        Ok(None) => Ok("<unknown>".to_string()),
        Err(e) => Err(Error::SqliteError(e)),
    }
}

/// Generate debug output to be fed into an external script to examine query plans.
/// TODO: it uses mocked arguments, which it assumes are strings. This does not always result in a
/// valid query.
#[cfg(test)]
fn log_sql_eqp(conn: &Connection, sql_query: &str) {
    if std::env::var("BLOCKSTACK_DB_TRACE") != Ok("1".to_string()) {
        return;
    }

    let mut parts = sql_query.split(" ");
    let mut full_sql = if let Some(part) = parts.next() {
        part.to_string()
    } else {
        sql_query.to_string()
    };

    while let Some(part) = parts.next() {
        if part.starts_with("?") {
            full_sql = format!("{} \"mock_arg\"", full_sql.trim());
        } else {
            full_sql = format!("{} {}", full_sql.trim(), part.trim());
        }
    }

    let path = get_db_path(conn).unwrap_or("ERROR!".to_string());
    let eqp_sql = format!("\"{}\" EXPLAIN QUERY PLAN {}", &path, full_sql.trim());
    wrb_debug!("{}", &eqp_sql);
}

#[cfg(not(test))]
fn log_sql_eqp(_conn: &Connection, _sql_query: &str) {}

/// boilerplate code for querying rows
pub fn query_rows<T, P>(conn: &Connection, sql_query: &str, sql_args: P) -> Result<Vec<T>, Error>
where
    P: IntoIterator + rusqlite::Params,
    P::Item: ToSql,
    T: FromRow<T>,
{
    log_sql_eqp(conn, sql_query);
    let mut stmt = conn.prepare(sql_query)?;
    let result = stmt.query_and_then(sql_args, |row| T::from_row(row))?;

    result.collect()
}

/// boilerplate code for querying a single row
///   if more than 1 row is returned, excess rows are ignored.
pub fn query_row<T, P>(conn: &Connection, sql_query: &str, sql_args: P) -> Result<Option<T>, Error>
where
    P: IntoIterator + rusqlite::Params,
    P::Item: ToSql,
    T: FromRow<T>,
{
    log_sql_eqp(conn, sql_query);
    let query_result = conn.query_row_and_then(sql_query, sql_args, |row| T::from_row(row));
    match query_result {
        Ok(x) => Ok(Some(x)),
        Err(Error::SqliteError(sqlite_error::QueryReturnedNoRows)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// boilerplate code for querying a single row
///   if more than 1 row is returned, panic
pub fn query_expect_row<T, P>(
    conn: &Connection,
    sql_query: &str,
    sql_args: P,
) -> Result<Option<T>, Error>
where
    P: IntoIterator + rusqlite::Params,
    P::Item: ToSql,
    T: FromRow<T>,
{
    log_sql_eqp(conn, sql_query);
    let mut stmt = conn.prepare(sql_query)?;
    let mut result = stmt.query_and_then(sql_args, |row| T::from_row(row))?;
    let mut return_value = None;
    if let Some(value) = result.next() {
        return_value = Some(value?);
    }
    assert!(
        result.next().is_none(),
        "FATAL: Multiple values returned for query that expected a single result:\n {}",
        sql_query
    );
    Ok(return_value)
}

pub fn query_row_panic<T, P, F>(
    conn: &Connection,
    sql_query: &str,
    sql_args: P,
    panic_message: F,
) -> Result<Option<T>, Error>
where
    P: IntoIterator + rusqlite::Params,
    P::Item: ToSql,
    T: FromRow<T>,
    F: FnOnce() -> String,
{
    log_sql_eqp(conn, sql_query);
    let mut stmt = conn.prepare(sql_query)?;
    let mut result = stmt.query_and_then(sql_args, |row| T::from_row(row))?;
    let mut return_value = None;
    if let Some(value) = result.next() {
        return_value = Some(value?);
    }
    if result.next().is_some() {
        panic!("{}", &panic_message());
    }
    Ok(return_value)
}

/// boilerplate code for querying a column out of a sequence of rows
pub fn query_row_columns<T, P>(
    conn: &Connection,
    sql_query: &str,
    sql_args: P,
    column_name: &str,
) -> Result<Vec<T>, Error>
where
    P: IntoIterator + rusqlite::Params,
    P::Item: ToSql,
    T: FromColumn<T>,
{
    log_sql_eqp(conn, sql_query);
    let mut stmt = conn.prepare(sql_query)?;
    let mut rows = stmt.query(sql_args)?;

    // gather
    let mut row_data = vec![];
    while let Some(row) = rows.next().map_err(|e| Error::SqliteError(e))? {
        let next_row = T::from_column(&row, column_name)?;
        row_data.push(next_row);
    }

    Ok(row_data)
}

/// Boilerplate for querying a single integer (first and only item of the query must be an int)
pub fn query_int<P>(conn: &Connection, sql_query: &str, sql_args: P) -> Result<i64, Error>
where
    P: IntoIterator + rusqlite::Params,
    P::Item: ToSql,
{
    log_sql_eqp(conn, sql_query);
    let mut stmt = conn.prepare(sql_query)?;
    let mut rows = stmt.query(sql_args)?;
    let mut row_data = vec![];
    while let Some(row) = rows.next().map_err(|e| Error::SqliteError(e))? {
        if row_data.len() > 0 {
            return Err(Error::Overflow);
        }
        let i: i64 = row.get_unwrap(0);
        row_data.push(i);
    }

    if row_data.len() == 0 {
        return Err(Error::NotFoundError);
    }

    Ok(row_data[0])
}

pub fn query_count<P>(conn: &Connection, sql_query: &str, sql_args: P) -> Result<i64, Error>
where
    P: IntoIterator + rusqlite::Params,
    P::Item: ToSql,
{
    query_int(conn, sql_query, sql_args)
}

/// Run a PRAGMA statement.  This can't always be done via execute(), because it may return a result (and
/// rusqlite does not like this).
pub fn sql_pragma(
    conn: &Connection,
    pragma_name: &str,
    pragma_value: &dyn ToSql,
) -> Result<(), Error> {
    inner_sql_pragma(conn, pragma_name, pragma_value).map_err(|e| Error::SqliteError(e))
}

fn inner_sql_pragma(
    conn: &Connection,
    pragma_name: &str,
    pragma_value: &dyn ToSql,
) -> Result<(), sqlite_error> {
    conn.pragma_update(None, pragma_name, pragma_value)
}

/// Run a VACUUM command
pub fn sql_vacuum(conn: &Connection) -> Result<(), Error> {
    conn.execute("VACUUM", rusqlite::params![])
        .map_err(Error::SqliteError)
        .and_then(|_| Ok(()))
}

/// Returns true if the database table `table_name` exists in the active
///  database of the provided SQLite connection.
pub fn table_exists(conn: &Connection, table_name: &str) -> Result<bool, sqlite_error> {
    let sql = "SELECT name FROM sqlite_master WHERE type='table' AND name=?";
    conn.query_row(sql, &[table_name], |row| row.get::<_, String>(0))
        .optional()
        .map(|r| r.is_some())
}

pub fn tx_busy_handler(run_count: i32) -> bool {
    let mut sleep_count = 2;
    if run_count > 0 {
        sleep_count = 2u64.saturating_pow(run_count as u32);
    }
    sleep_count = sleep_count.saturating_add(thread_rng().gen::<u64>() % sleep_count);

    if sleep_count > 100 {
        let jitter = thread_rng().gen::<u64>() % 20;
        sleep_count = 100 - jitter;
    }

    wrb_debug!(
        "Database is locked; sleeping {}ms and trying again",
        &sleep_count
    );

    sleep_ms(sleep_count);
    true
}

/// Begin an immediate-mode transaction, and handle busy errors with exponential backoff.
/// Handling busy errors when the tx begins is preferable to doing it when the tx commits, since
/// then we don't have to worry about any extra rollback logic.
pub fn tx_begin_immediate<'a>(conn: &'a mut Connection) -> Result<DBTx<'a>, Error> {
    tx_begin_immediate_sqlite(conn).map_err(Error::from)
}

/// Begin an immediate-mode transaction, and handle busy errors with exponential backoff.
/// Handling busy errors when the tx begins is preferable to doing it when the tx commits, since
/// then we don't have to worry about any extra rollback logic.
/// Sames as `tx_begin_immediate` except that it returns a rusqlite error.
pub fn tx_begin_immediate_sqlite<'a>(conn: &'a mut Connection) -> Result<DBTx<'a>, sqlite_error> {
    conn.busy_handler(Some(tx_busy_handler))?;
    let tx = Transaction::new(conn, TransactionBehavior::Immediate)?;
    Ok(tx)
}

/// Open a database connection and set some typically-used pragmas
pub fn sqlite_open<P: AsRef<Path>>(
    path: P,
    flags: OpenFlags,
    foreign_keys: bool,
) -> Result<Connection, sqlite_error> {
    let db = Connection::open_with_flags(path, flags)?;
    db.busy_handler(Some(tx_busy_handler))?;
    inner_sql_pragma(&db, "journal_mode", &"WAL")?;
    inner_sql_pragma(&db, "synchronous", &"NORMAL")?;
    if foreign_keys {
        inner_sql_pragma(&db, "foreign_keys", &true)?;
    }
    Ok(db)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_pragma() {
        let path = "/tmp/wrb_db_test_pragma.db";
        if fs::metadata(path).is_ok() {
            fs::remove_file(path).unwrap();
        }

        // calls pragma_update with both journal_mode and foreign_keys
        let db = sqlite_open(
            path,
            OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_READ_WRITE,
            true,
        )
        .unwrap();

        // journal mode must be WAL
        db.pragma_query(None, "journal_mode", |row| {
            let value: String = row.get(0)?;
            assert_eq!(value, "wal");
            Ok(())
        })
        .unwrap();

        // foreign keys must be on
        db.pragma_query(None, "foreign_keys", |row| {
            let value: i64 = row.get(0)?;
            assert_eq!(value, 1);
            Ok(())
        })
        .unwrap();
    }
}
