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

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::io::{Read, Write};
use std::net::SocketAddr;

use rusqlite::Error as sqlite_error;

use stacks_common::util::secp256k1::Secp256k1PrivateKey;
use stacks_common::util::secp256k1::Secp256k1PublicKey;

use stacks_common::codec::Error as CodecError;
use stacks_common::codec::StacksMessageCodec;
use stacks_common::codec::{read_next, read_next_at_most, write_next};
use stacks_common::types::chainstate::StacksAddress;
use stacks_common::util::hash::Hash160;
use stacks_common::util::hash::{hex_bytes, to_hex};

use clarity::vm::types::QualifiedContractIdentifier;

use crate::runner::Error as RuntimeError;
use crate::util::sqlite::Error as DBError;
use libstackerdb::{SlotMetadata, StackerDBChunkAckData, StackerDBChunkData};

use serde;
use serde::de::SeqAccess;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize};

#[cfg(test)]
pub mod tests;

pub mod mock;
pub mod wrbpod;

pub const WRBPOD_SLICES_VERSION: u8 = 0;
pub const WRBPOD_SUPERBLOCK_VERSION: u8 = 0;
pub const WRBPOD_APP_STATE_VERSION: u8 = 0;

pub const WRBPOD_MAX_SLOTS: u32 = 4096; // same as maximum stackerdb size in the stacks node
pub const WRBPOD_CHUNK_MAX_SIZE: u32 = libstackerdb::STACKERDB_MAX_CHUNK_SIZE;

/// Chunks that make up a slot in a stackerdb.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WrbpodSlices {
    /// Version of this struct
    pub version: u8,
    /// Slices -- list of buffers to store
    #[serde(
        serialize_with = "wrbpod_slices_serialize",
        deserialize_with = "wrbpod_slices_deserialize"
    )]
    slices: Vec<Vec<u8>>,
    /// Slice indexes (maps clarity ID to slice index)
    index: BTreeMap<u128, usize>,
    /// Whether or not this chunk has unsaved changes
    dirty: bool,
    /// (not stored)
    encoded_size: u64,
    /// (not stored)
    max_size: u64,
}

fn wrbpod_slices_serialize<S: serde::Serializer>(
    slices: &Vec<Vec<u8>>,
    s: S,
) -> Result<S::Ok, S::Error> {
    let mut seq = s.serialize_seq(Some(slices.len()))?;
    for slice in slices {
        let slice_hex = to_hex(&slice);
        seq.serialize_element(slice_hex.as_str())?;
    }
    seq.end()
}

struct WrbpodSlicesDeserializeVisitor();

impl<'de> serde::de::Visitor<'de> for WrbpodSlicesDeserializeVisitor {
    type Value = Vec<Vec<u8>>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("WrbpodSlicesDeserialize hex string sequence")
    }

    fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
    where
        S: SeqAccess<'de>,
    {
        let mut slices = vec![];
        while let Some(slice_hex) = seq.next_element::<String>()? {
            let slice_bytes = hex_bytes(&slice_hex).map_err(serde::de::Error::custom)?;
            slices.push(slice_bytes);
        }
        Ok(slices)
    }
}

fn wrbpod_slices_deserialize<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<Vec<u8>>, D::Error> {
    let slices = d.deserialize_seq(WrbpodSlicesDeserializeVisitor())?;
    Ok(slices)
}

/// Control state for an application.
/// Part of the Wrb superblock
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WrbpodAppState {
    pub version: u8,
    pub code_hash: Hash160,
    pub slots: Vec<u32>,
}

/// Free list
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WrbpodSlot {
    Filled(u32),
    Free(u32),
}

impl WrbpodSlot {
    pub fn slot_id(&self) -> u32 {
        match self {
            Self::Filled(x) => *x,
            Self::Free(x) => *x,
        }
    }

    pub fn is_free(&self) -> bool {
        match self {
            Self::Filled(..) => false,
            Self::Free(..) => true,
        }
    }

    pub fn as_filled(&self) -> Self {
        Self::Filled(self.slot_id())
    }

    pub fn as_free(&self) -> Self {
        Self::Free(self.slot_id())
    }
}

impl StacksMessageCodec for WrbpodSlot {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), CodecError> {
        match self {
            Self::Filled(x) => {
                write_next(fd, &1u8)?;
                write_next(fd, x)?;
            }
            Self::Free(x) => {
                write_next(fd, &0u8)?;
                write_next(fd, x)?;
            }
        }
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<Self, CodecError> {
        let filled: u8 = read_next(fd)?;
        match filled {
            x if x == 0 => {
                let slot_id: u32 = read_next(fd)?;
                Ok(Self::Free(slot_id))
            }
            x if x == 1 => {
                let slot_id: u32 = read_next(fd)?;
                Ok(Self::Filled(slot_id))
            }
            _ => Err(CodecError::DeserializeError(
                "Invalid fill/free byte".into(),
            )),
        }
    }
}

/// Control structure for a wrbpod.
/// This gets written to slot 0.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WrbpodSuperblock {
    /// version of this struct
    pub version: u8,
    /// list of StackerDB slots allocated to this wrbpod, as well as whether or not they're free
    pub slot_ids: Vec<WrbpodSlot>,
    /// which domains have which slots
    pub apps: BTreeMap<String, WrbpodAppState>,
}

/// StackerDB client trait (so we can mock it in testing)
pub trait StackerDBClient: Send {
    /// Address of the stackerdb host we're talking to
    fn get_host(&self) -> SocketAddr;

    /// List all chunk metadata for the stackerdb
    fn list_chunks(&mut self) -> Result<Vec<SlotMetadata>, RuntimeError>;

    /// Get one or more chunks with specific versions.
    /// The ith entry in slots_and_versions corresponds to the ith item in the returned list.  A
    /// Some(..) value indicates that the chunk with that version exists.  A None value indicates
    /// that a chunk with that version does not exist.
    fn get_chunks(
        &mut self,
        slots_and_versions: &[(u32, u32)],
    ) -> Result<Vec<Option<Vec<u8>>>, RuntimeError>;

    /// Get one or more chunks regardless of what their versions are.
    /// The ith entry in slots_ids corresponds to the ith item in the returned list.  A
    /// Some(..) value indicates that the chunk exists.  A None value indicates
    /// that a chunk does not exist.
    fn get_latest_chunks(&mut self, slot_ids: &[u32])
        -> Result<Vec<Option<Vec<u8>>>, RuntimeError>;

    /// Upload a given chunk to the stackerdb
    fn put_chunk(
        &mut self,
        chunk: StackerDBChunkData,
    ) -> Result<StackerDBChunkAckData, RuntimeError>;

    /// Find a list of nodes that host this stackerdb.  These will be p2p addresses.
    fn find_replicas(&mut self) -> Result<Vec<SocketAddr>, RuntimeError>;

    /// Get the list of signers for the replica.
    fn get_signers(&mut self) -> Result<Vec<StacksAddress>, RuntimeError>;
}

/// Instantiated handle to a Wrbpod
pub struct Wrbpod {
    /// top-level control structure
    superblock: WrbpodSuperblock,
    /// connection to a node with a replica
    replica_client: Box<dyn StackerDBClient>,
    /// connection to the home node
    home_client: Box<dyn StackerDBClient>,
    /// signing key
    privkey: Secp256k1PrivateKey,
    /// cached copy of the signers in the DB
    signers: Option<Vec<StacksAddress>>,
    /// Maps stackerdb slot ID to slices
    chunks: HashMap<u32, WrbpodSlices>,
    /// which slot ID contains the superblock
    superblock_slot_id: u32,
}

unsafe impl Send for Wrbpod {}

#[derive(Clone, PartialEq, Hash, Eq)]
pub struct WrbpodAddress {
    pub contract: QualifiedContractIdentifier,
    pub slot: u32,
}

impl WrbpodAddress {
    pub fn new(contract: QualifiedContractIdentifier, slot: u32) -> Self {
        Self { contract, slot }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let mut parts_iter = s.split("/");
        let qc_str = parts_iter.next()?;
        let slot_str = parts_iter.next()?;
        let contract = QualifiedContractIdentifier::parse(qc_str).ok()?;
        let slot = slot_str.parse::<u32>().ok()?;
        Some(Self { contract, slot })
    }

    fn inner_to_string(&self) -> String {
        format!("{}/{}", &self.contract, self.slot)
    }
}

impl fmt::Debug for WrbpodAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.inner_to_string())
    }
}

impl fmt::Display for WrbpodAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.inner_to_string())
    }
}

#[derive(Debug)]
pub enum Error {
    Runtime(RuntimeError),
    Codec(CodecError),
    GetChunk(String),
    PutChunk(String),
    Overflow(String),
    NoSuchChunk,
    NoSpace,
    NoSuperblock,
    NoSuchRow,
    AlreadyExists,
    DBError(DBError),
}

impl From<RuntimeError> for Error {
    fn from(e: RuntimeError) -> Self {
        Self::Runtime(e)
    }
}

impl From<CodecError> for Error {
    fn from(e: CodecError) -> Self {
        Self::Codec(e)
    }
}

impl From<DBError> for Error {
    fn from(e: DBError) -> Self {
        Self::DBError(e)
    }
}

impl From<sqlite_error> for Error {
    fn from(e: sqlite_error) -> Self {
        Self::DBError(DBError::SqliteError(e))
    }
}

impl StacksMessageCodec for WrbpodAppState {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), CodecError> {
        write_next(fd, &self.version)?;
        write_next(fd, &self.code_hash)?;
        write_next(fd, &self.slots)?;
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<Self, CodecError> {
        let version: u8 = read_next(fd)?;
        let code_hash: Hash160 = read_next(fd)?;
        let slots: Vec<u32> = read_next(fd)?;
        Ok(Self {
            version,
            code_hash,
            slots,
        })
    }
}

fn u128_consensus_serialize<W: Write>(fd: &mut W, value: u128) -> Result<(), CodecError> {
    let bytes = value.to_be_bytes();
    fd.write_all(&bytes)
        .map_err(|e| CodecError::SerializeError(format!("Failed to write u128: {:?}", &e)))?;
    Ok(())
}

fn u128_consensus_deserialize<R: Read>(fd: &mut R) -> Result<u128, CodecError> {
    let mut bytes = [0u8; 16];
    fd.read_exact(&mut bytes)
        .map_err(|e| CodecError::DeserializeError(format!("Failed to read u128: {:?}", &e)))?;
    Ok(u128::from_be_bytes(bytes))
}

impl StacksMessageCodec for WrbpodSlices {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), CodecError> {
        write_next(fd, &self.version)?;

        // write index
        let count_u64 = u64::try_from(self.index.len())
            .map_err(|_| CodecError::SerializeError("Failed to convert index len to u64".into()))?;
        write_next(fd, &count_u64)?;
        for (id, idx) in self.index.iter() {
            let idx_u64 = u64::try_from(*idx)
                .map_err(|_| CodecError::SerializeError("Failed to convert usize to u64".into()))?;
            u128_consensus_serialize(fd, *id)?;
            write_next(fd, &idx_u64)?;
        }

        // write slices
        for slice in self.slices.iter() {
            write_next(fd, slice)?;
        }
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<Self, CodecError> {
        let mut encoded_size = 0;
        let version: u8 = read_next(fd)?;
        encoded_size += 1;

        let index_count: u64 = read_next(fd)?;
        encoded_size += 8;

        let mut index = BTreeMap::new();
        for _ in 0..index_count {
            let id = u128_consensus_deserialize(fd)?;
            encoded_size += 16;

            let idx_u64: u64 = read_next(fd)?;
            encoded_size += 8;

            if idx_u64 >= index_count {
                return Err(CodecError::DeserializeError(format!(
                    "index {} exceeds number of slices {}",
                    idx_u64, index_count
                )))?;
            }
            let idx = usize::try_from(idx_u64).map_err(|_| {
                CodecError::DeserializeError("Failed to convert u64 to usize".into())
            })?;
            if index.get(&id).is_some() {
                return Err(CodecError::DeserializeError("duplicate slice ID".into()))?;
            }
            index.insert(id, idx);
        }
        let mut slices = vec![];
        for _ in 0..index_count {
            let slice: Vec<u8> = read_next(fd)?;
            encoded_size += 4 + (u64::try_from(slice.len()).expect("slice too big"));
            slices.push(slice);
        }

        Ok(Self {
            version,
            index,
            slices,
            dirty: false,
            encoded_size,
            max_size: WRBPOD_CHUNK_MAX_SIZE.into(),
        })
    }
}

impl StacksMessageCodec for WrbpodSuperblock {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), CodecError> {
        write_next(fd, &self.version)?;
        write_next(fd, &self.slot_ids)?;
        let mut bns_names = Vec::with_capacity(self.apps.len());
        for (bns_name, _) in self.apps.iter() {
            bns_names.push(bns_name.as_bytes().to_vec());
        }
        write_next(fd, &bns_names)?;
        for (_, app_state) in self.apps.iter() {
            write_next(fd, app_state)?;
        }
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<Self, CodecError> {
        let version: u8 = read_next(fd)?;
        let slot_ids: Vec<WrbpodSlot> = read_next_at_most(fd, WRBPOD_MAX_SLOTS)?;
        let bns_names_bytes: Vec<Vec<u8>> = read_next(fd)?;
        let mut bns_names = vec![];
        for bns_name_bytes in bns_names_bytes.into_iter() {
            let bns_name = std::str::from_utf8(&bns_name_bytes)
                .map_err(|_| CodecError::DeserializeError("BNS name is not UTF-8".into()))?;
            if !bns_name.is_ascii() {
                return Err(CodecError::DeserializeError("BNS name is not ASCII".into()));
            }
            bns_names.push(bns_name.to_string());
        }

        let mut app_state = BTreeMap::new();
        for bns_name in bns_names.into_iter() {
            app_state.insert(bns_name, read_next(fd)?);
        }
        Ok(Self {
            version,
            apps: app_state,
            slot_ids,
        })
    }
}
