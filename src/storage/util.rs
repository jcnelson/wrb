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

use std::io;
use std::fs;
use std::path::PathBuf;

use rusqlite::{Connection, OpenFlags, NO_PARAMS};

use crate::util::sqlite::FromColumn;
use crate::util::sqlite::sqlite_open;

use stacks_common::types::chainstate::StacksBlockId;

use crate::vm::BOOT_BLOCK_ID;

/*
/// Get the DB path for the wrb local state, given the top-level directory
pub fn get_wrb_headers_db_path(db_path: &str, domain: &str) -> String {
    if db_path == ":memory:" {
        return db_path.to_string();
    }

    let mut wrb_db_path_buf = PathBuf::from(db_path);
    wrb_db_path_buf.push(domain);
    wrb_db_path_buf.push("wrb.sqlite");
    let wrb_db_path = wrb_db_path_buf
        .to_str()
        .expect(&format!(
            "FATAL: failed to convert '{}' to a string",
            db_path
        ))
        .to_string();
    wrb_db_path
}
*/


/// Get the next chain tip for the wrb headers DB
pub fn make_wrb_chain_tip(cur_height: u64) -> StacksBlockId {
    let mut bytes = [0u8; 32];
    bytes[0..8].copy_from_slice(&cur_height.to_be_bytes());
    StacksBlockId(bytes)
}

/// Get the chain tip of the wrb headers DB
pub fn get_wrb_chain_tip(conn: &Connection) -> StacksBlockId {
    let mut stmt = conn.prepare("SELECT chain_tip FROM kvstore ORDER BY height DESC, chain_tip ASC LIMIT 1")
        .expect("FATAL: could not prepare query");
    let mut rows = stmt.query(NO_PARAMS)
        .expect("FATAL: could not fetch rows");
    let mut hash_opt = None;
    while let Some(row) = rows.next().expect("FATAL: could not read block hash") {
        let bhh = StacksBlockId::from_column(&row, "chain_tip")
            .expect("FATAL: could not parse block hash");
        if bhh == BOOT_BLOCK_ID {
            continue;
        }
        hash_opt = Some(bhh);
        break;
    }
    match hash_opt {
        Some(bhh) => bhh,
        None => BOOT_BLOCK_ID.clone()
    }
}

/// Get the block height of the wrb headers DB
pub fn get_wrb_block_height(conn: &Connection, block_id: &StacksBlockId) -> Option<u64> {
    let mut stmt = conn.prepare("SELECT height FROM kvstore WHERE chain_tip = ?1")
        .expect("FATAL: could not prepare query");
    
    let mut rows = stmt.query(&[block_id])
        .expect("FATAL: could not fetch rows");

    let mut height_opt = None;

    while let Some(row) = rows.next().expect("FATAL: could not read block hash") {
        let height = u64::from_column(&row, "height")
            .expect("FATAL: could not parse row ID");

        height_opt = Some(height);
        break;
    }

    height_opt
}

/*
/// Create or open a db path
pub fn create_or_open_db(path: &String) -> Connection {
    let open_flags = if path == ":memory:" {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    } else {
        match fs::metadata(path) {
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    // need to create
                    if let Some(dirp) = PathBuf::from(path).parent() {
                        fs::create_dir_all(dirp).unwrap_or_else(|e| {
                            eprintln!("Failed to create {:?}: {:?}", dirp, &e);
                            panic!();
                        });
                    }
                    OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
                } else {
                    panic!("FATAL: could not stat {}", path);
                }
            }
            Ok(_md) => {
                // can just open
                OpenFlags::SQLITE_OPEN_READ_WRITE
            }
        }
    };

    let conn = sqlite_open(path, open_flags, false)
        .expect(&format!("FATAL: failed to open '{}'", path));
    conn
}
*/
