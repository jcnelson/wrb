// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2023 Stacks Open Internet Foundation
// Copyright (C) 2023 Jude Nelson
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
use std::fs;
use std::net::SocketAddr;

use rusqlite::Connection;
use rusqlite::OpenFlags;
use rusqlite::OptionalExtension;
use rusqlite::Row;
use rusqlite::Transaction;

use crate::runner::Error as RuntimeError;
use crate::storage::Error;
use crate::storage::StackerDBClient;
use crate::storage::WrbpodSlices;
use crate::storage::WRBPOD_SLICES_VERSION;

use crate::util::sqlite::Error as DBError;
use crate::util::sqlite::FromRow;

use crate::ui::Renderer;

use crate::vm::ClarityVM;

use stacks_common::address::{
    c32::c32_address_decode, C32_ADDRESS_VERSION_MAINNET_SINGLESIG,
    C32_ADDRESS_VERSION_TESTNET_SINGLESIG,
};
use stacks_common::types::chainstate::StacksAddress;
use stacks_common::types::chainstate::StacksPrivateKey;
use stacks_common::types::chainstate::StacksPublicKey;
use stacks_common::util::hash::to_hex;
use stacks_common::util::hash::Hash160;
use stacks_common::util::hash::Sha512Trunc256Sum;
use stacks_common::util::secp256k1::MessageSignature;
use stacks_common::util::sleep_ms;

use crate::util::sqlite::{query_row, query_rows, sqlite_open, tx_begin_immediate, u64_to_sql};

use libstackerdb::{SlotMetadata, StackerDBChunkAckData, StackerDBChunkData};

use crate::core;
use crate::core::Config;
use crate::runner::Runner;

use serde::{de::Error as de_Error, Deserialize, Serialize};

const SCHEMA_VERSION: &'static str = "1";

const LOCAL_STACKERDB_SCHEMA: &'static [&'static str] = &[
    r#"
    CREATE TABLE IF NOT EXISTS chunks(
        slot_id INTEGER PRIMARY KEY,
        slot_version INTEGER NOT NULL,
        pubkh TEXT NOT NULL,
        data_hash TEXT NOT NULL,
        data BLOB NOT NULL,
        signature TEXT NOT NULL
    );"#,
    r#"
    CREATE TABLE IF NOT EXISTS config(
        max_slots INTEGER NOT NULL,
        rpc_latency INTEGER NOT NULL,
        mainnet BOOLEAN NOT NULL
    );"#,
    r#"
    CREATE TABLE IF NOT EXISTS schema_version(
        version INTEGER NOT NULL
    );
    "#,
    r#"
    CREATE INDEX by_id_and_version ON chunks(slot_id,slot_version);
    "#,
    r#"
    INSERT INTO schema_version (version) VALUES (1);
    "#,
];

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
struct LocalStackerDBChunkMetadata {
    slot_id: u32,
    slot_version: u32,
    pubkh: Hash160,
    data_hash: Sha512Trunc256Sum,
    signature: MessageSignature,
}

trait SlotMetadataIsEmpty {
    fn is_empty(&self) -> bool;
}

impl SlotMetadataIsEmpty for LocalStackerDBChunkMetadata {
    fn is_empty(&self) -> bool {
        self.slot_version == 0 && self.data_hash == Sha512Trunc256Sum([0x00; 32])
    }
}

impl SlotMetadataIsEmpty for SlotMetadata {
    fn is_empty(&self) -> bool {
        self.slot_version == 0 && self.data_hash == Sha512Trunc256Sum([0x00; 32])
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
struct LocalStackerDBChunk {
    slot_id: u32,
    slot_version: u32,
    pubkh: Hash160,
    data_hash: Sha512Trunc256Sum,
    data: Vec<u8>,
    signature: MessageSignature,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Signer {
    #[serde(
        serialize_with = "signer_address_serialize",
        deserialize_with = "signer_address_deserialize"
    )]
    pub address: StacksAddress,
    pub num_slots: u32,
}

fn signer_address_serialize<S: serde::Serializer>(
    address: &StacksAddress,
    s: S,
) -> Result<S::Ok, S::Error> {
    let txt = address.to_string();
    s.serialize_str(&txt)
}

fn signer_address_deserialize<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<StacksAddress, D::Error> {
    let txt = String::deserialize(d)?;

    let (version, bytes) = c32_address_decode(&txt).map_err(de_Error::custom)?;
    if bytes.len() != 20 {
        return Err(de_Error::custom("Expected 20 bytes"));
    }
    let mut bytes_20 = [0u8; 20];
    bytes_20.copy_from_slice(&bytes[0..20]);

    let addr = StacksAddress::new(version, Hash160(bytes_20)).map_err(de_Error::custom)?;
    Ok(addr)
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct LocalStackerDBConfig {
    pub max_slots: u32,
    pub rpc_latency: u64,
    pub mainnet: bool,
    pub signers: Vec<Signer>,
}

impl FromRow<LocalStackerDBChunkMetadata> for LocalStackerDBChunkMetadata {
    fn from_row<'a>(row: &'a Row) -> Result<Self, DBError> {
        let slot_id: u32 = row.get("slot_id")?;
        let slot_version: u32 = row.get("slot_version")?;
        let pubkh_str: String = row.get("pubkh")?;
        let pubkh: Hash160 = Hash160::from_hex(&pubkh_str).map_err(|_| DBError::ParseError)?;
        let data_hash_str: String = row.get("data_hash")?;
        let data_hash: Sha512Trunc256Sum =
            Sha512Trunc256Sum::from_hex(&data_hash_str).map_err(|_| DBError::ParseError)?;
        let signature_str: String = row.get("signature")?;
        let signature =
            MessageSignature::from_hex(&signature_str).map_err(|_| DBError::ParseError)?;
        Ok(Self {
            slot_id,
            slot_version,
            pubkh,
            data_hash,
            signature,
        })
    }
}

impl FromRow<LocalStackerDBChunk> for LocalStackerDBChunk {
    fn from_row<'a>(row: &'a Row) -> Result<Self, DBError> {
        let md = LocalStackerDBChunkMetadata::from_row(row)?;
        let data: Vec<u8> = row.get("data")?;
        Ok(Self {
            slot_id: md.slot_id,
            slot_version: md.slot_version,
            pubkh: md.pubkh,
            data_hash: md.data_hash,
            signature: md.signature,
            data,
        })
    }
}

impl FromRow<LocalStackerDBConfig> for LocalStackerDBConfig {
    fn from_row<'a>(row: &'a Row) -> Result<Self, DBError> {
        let max_slots: u32 = row.get("max_slots")?;
        let rpc_latency: u32 = row.get("rpc_latency")?;
        let mainnet: bool = row.get("mainnet")?;
        Ok(Self {
            max_slots,
            mainnet,
            rpc_latency: u64::from(rpc_latency),
            signers: vec![],
        })
    }
}

pub struct LocalStackerDBClient {
    pub path: String,
    conn: Connection,
}

impl LocalStackerDBClient {
    pub fn open(path: &str) -> Result<Self, Error> {
        if path != ":memory:" && !std::fs::metadata(path).is_ok() {
            return Err(Error::AlreadyExists);
        }
        let conn = sqlite_open(path, OpenFlags::SQLITE_OPEN_READ_WRITE, true)?;
        Ok(Self {
            path: path.to_string(),
            conn,
        })
    }

    pub fn open_or_create(path: &str, config: LocalStackerDBConfig) -> Result<Self, Error> {
        let (create, open_flags) = if path != ":memory:" && std::fs::metadata(path).is_ok() {
            (false, OpenFlags::SQLITE_OPEN_READ_WRITE)
        } else {
            (
                true,
                OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_READ_WRITE,
            )
        };

        let mut conn = sqlite_open(path, open_flags, true)?;

        if !create {
            return Ok(Self {
                path: path.to_string(),
                conn,
            });
        }

        let tx = tx_begin_immediate(&mut conn)?;

        wrb_debug!("Instantiate LocalStackerDBClient at {}", path);

        for cmd in LOCAL_STACKERDB_SCHEMA.iter() {
            tx.execute(cmd, rusqlite::params![])?;
        }

        let mut slot_id = 0;
        for signer in config.signers.iter() {
            for _ in 0..signer.num_slots {
                tx.execute(
                    "INSERT INTO chunks (slot_id,slot_version,pubkh,data_hash,data,signature) VALUES (?1,?2,?3,?4,?5,?6)",
                    rusqlite::params![slot_id, 0, &signer.address.bytes().to_hex(), &Sha512Trunc256Sum([0x00; 32]).to_hex(), vec![], &MessageSignature::empty().to_hex()]
                )?;
                slot_id += 1;
            }
        }

        tx.execute(
            "INSERT INTO config (max_slots,rpc_latency,mainnet) VALUES (?1,?2,?3)",
            rusqlite::params![config.max_slots, config.rpc_latency, config.mainnet],
        )?;

        tx.commit()?;
        Ok(Self {
            path: path.to_string(),
            conn,
        })
    }

    pub fn tx_begin<'a>(&'a mut self) -> Result<Transaction<'a>, Error> {
        Ok(tx_begin_immediate(&mut self.conn)?)
    }

    fn inner_get_config(conn: &Connection) -> Result<LocalStackerDBConfig, Error> {
        match query_row(conn, "SELECT * FROM config", rusqlite::params![]) {
            Ok(Some(conf)) => Ok(conf),
            Ok(None) => Err(Error::NoSuchRow),
            Err(e) => Err(e.into()),
        }
    }

    fn inner_list_chunks(conn: &Connection) -> Result<Vec<SlotMetadata>, Error> {
        let sql = "SELECT slot_id,slot_version,pubkh,data_hash,signature FROM chunks ORDER BY slot_id ASC";
        let args = rusqlite::params![];
        let mds: Vec<LocalStackerDBChunkMetadata> = query_rows(conn, sql, args)?;
        Ok(mds
            .into_iter()
            .map(|md| SlotMetadata {
                slot_id: md.slot_id,
                slot_version: md.slot_version,
                data_hash: md.data_hash,
                signature: md.signature,
            })
            .collect())
    }
}

impl StackerDBClient for LocalStackerDBClient {
    fn get_host(&self) -> SocketAddr {
        let addr: SocketAddr = "127.0.0.1:30443".parse().unwrap();
        addr
    }

    fn list_chunks(&mut self) -> Result<Vec<SlotMetadata>, RuntimeError> {
        let config = Self::inner_get_config(&self.conn)?;
        sleep_ms(config.rpc_latency);

        Ok(Self::inner_list_chunks(&self.conn)?)
    }

    fn get_chunks(
        &mut self,
        slots_and_versions: &[(u32, u32)],
    ) -> Result<Vec<Option<Vec<u8>>>, RuntimeError> {
        let config = Self::inner_get_config(&self.conn)?;
        sleep_ms(config.rpc_latency);

        // make it seem like chunks with 0'ed hashes don't exist
        let mds: HashMap<u32, SlotMetadata> = Self::inner_list_chunks(&self.conn)?
            .into_iter()
            .map(|md| (md.slot_id, md))
            .collect();

        let sql = "SELECT data FROM chunks WHERE slot_id = ?1 AND slot_version = ?2";
        let mut ret = vec![];
        for (slot_id, slot_version) in slots_and_versions.iter() {
            let chunk_opt: Option<Vec<u8>> = self
                .conn
                .query_row(sql, rusqlite::params![*slot_id, *slot_version], |row| {
                    row.get(0)
                })
                .optional()
                .map_err(|e| RuntimeError::Database(e.to_string()))?;

            let chunk_opt = if let Some(chunk_md) = mds.get(slot_id) {
                if chunk_md.is_empty() {
                    // doesn't exist
                    None
                } else {
                    chunk_opt
                }
            } else {
                chunk_opt
            };

            ret.push(chunk_opt);
        }
        wrb_test_debug!("get_chunks({:?}): {:?}", slots_and_versions, &ret);
        Ok(ret)
    }

    fn get_latest_chunks(
        &mut self,
        slot_ids: &[u32],
    ) -> Result<Vec<Option<Vec<u8>>>, RuntimeError> {
        let config = Self::inner_get_config(&self.conn)?;
        sleep_ms(config.rpc_latency);

        // make it seem like chunks with 0'ed hashes don't exist
        let mds: HashMap<u32, SlotMetadata> = Self::inner_list_chunks(&self.conn)?
            .into_iter()
            .map(|md| (md.slot_id, md))
            .collect();

        let sql = "SELECT data FROM chunks WHERE slot_id = ?1";
        let mut ret = vec![];
        for slot_id in slot_ids.iter() {
            let chunk_opt: Option<Vec<u8>> = self
                .conn
                .query_row(sql, rusqlite::params![*slot_id], |row| {
                    let data: Vec<u8> = row.get("data")?;
                    Ok(data)
                })
                .optional()
                .map_err(|e| RuntimeError::Database(e.to_string()))?;

            let chunk_opt = if let Some(chunk_md) = mds.get(slot_id) {
                if chunk_md.is_empty() {
                    // doesn't exist
                    None
                } else {
                    chunk_opt
                }
            } else {
                chunk_opt
            };

            ret.push(chunk_opt);
        }
        wrb_test_debug!("get_latest_chunks({:?}): {:?}", slot_ids, &ret);
        Ok(ret)
    }

    fn put_chunk(
        &mut self,
        chunk: StackerDBChunkData,
    ) -> Result<StackerDBChunkAckData, RuntimeError> {
        let config = Self::inner_get_config(&self.conn)?;
        sleep_ms(config.rpc_latency);

        let tx = self.tx_begin()?;

        let config = Self::inner_get_config(&tx)?;
        if chunk.slot_id >= config.max_slots {
            let ret = StackerDBChunkAckData {
                accepted: false,
                reason: Some(format!(
                    "Slot {} too big (max is {})",
                    chunk.slot_id, config.max_slots
                )),
                metadata: None,
                code: Some(1),
            };
            return Ok(ret);
        }

        let sql = "SELECT slot_id,slot_version,pubkh,data_hash,signature FROM chunks WHERE slot_id = ?1 ORDER BY slot_id ASC";
        let Some(metadata): Option<LocalStackerDBChunkMetadata> =
            query_row(&tx, sql, rusqlite::params![chunk.slot_id])?
        else {
            let ret = StackerDBChunkAckData {
                accepted: false,
                reason: Some(format!("No such slot {}", chunk.slot_id)),
                metadata: None,
                code: Some(1),
            };
            return Ok(ret);
        };

        let slot_metadata = SlotMetadata {
            slot_id: metadata.slot_id,
            slot_version: metadata.slot_version,
            data_hash: metadata.data_hash.clone(),
            signature: metadata.signature.clone(),
        };

        if !slot_metadata.is_empty()
            && !slot_metadata
                .verify(&StacksAddress::new(0, metadata.pubkh).expect("Infallible"))
                .unwrap_or(false)
        {
            let ret = StackerDBChunkAckData {
                accepted: false,
                reason: Some(format!("Slot {}: bad signature", chunk.slot_id)),
                metadata: Some(slot_metadata),
                code: Some(2),
            };
            return Ok(ret);
        }

        if !slot_metadata.is_empty() && chunk.slot_version <= slot_metadata.slot_version {
            let ret = StackerDBChunkAckData {
                accepted: false,
                reason: Some(format!(
                    "Slot {}: version {} <= {}",
                    chunk.slot_id, chunk.slot_version, slot_metadata.slot_version
                )),
                metadata: Some(slot_metadata),
                code: Some(0),
            };
            return Ok(ret);
        }

        let sql = "UPDATE chunks SET slot_version = ?1, data_hash = ?2, data = ?3, signature = ?4 WHERE slot_id = ?5";
        let args = rusqlite::params![
            chunk.slot_version,
            &Sha512Trunc256Sum::from_data(&chunk.data).to_hex(),
            &chunk.data,
            &chunk.sig.to_hex(),
            chunk.slot_id
        ];
        tx.execute(sql, args)?;
        tx.commit()?;

        let ret = StackerDBChunkAckData {
            accepted: true,
            reason: None,
            metadata: None,
            code: None,
        };
        wrb_test_debug!("put_chunk({:?}): {:?}", &chunk, &ret);
        Ok(ret)
    }

    fn find_replicas(&mut self) -> Result<Vec<SocketAddr>, RuntimeError> {
        let config = Self::inner_get_config(&self.conn)?;
        sleep_ms(config.rpc_latency);

        return Ok(vec![SocketAddr::from(([127, 0, 0, 1], 20443))]);
    }

    fn get_signers(&mut self) -> Result<Vec<StacksAddress>, RuntimeError> {
        let config = Self::inner_get_config(&self.conn)?;
        sleep_ms(config.rpc_latency);

        let addr_version = if config.mainnet {
            C32_ADDRESS_VERSION_MAINNET_SINGLESIG
        } else {
            C32_ADDRESS_VERSION_TESTNET_SINGLESIG
        };

        let sql = "SELECT slot_id,slot_version,pubkh,data_hash,signature FROM chunks ORDER BY slot_id ASC";
        let args = rusqlite::params![];
        let mds: Vec<LocalStackerDBChunkMetadata> = query_rows(&self.conn, sql, args)?;

        Ok(mds
            .into_iter()
            .map(|md| StacksAddress::new(addr_version, md.pubkh).expect("infallible"))
            .collect())
    }
}
