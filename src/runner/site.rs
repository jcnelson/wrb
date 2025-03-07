// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2022 Stacks Open Internet Foundation
// Copyright (C) 2022-2025 Jude Nelson
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

use std::net::SocketAddr;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::env;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use base64ct::{Base64, Encoding};

use clarity::vm::types::PrincipalData;
use clarity::vm::types::QualifiedContractIdentifier;

use crate::core::Config;

use crate::runner;
use crate::runner::bns::BNSResolver;
use crate::runner::stackerdb::StackerDBSession;
use crate::runner::Error;
use crate::runner::Runner;

use crate::storage::StackerDBClient;

use stacks_common::codec::{read_next, write_next, Error as CodecError, StacksMessageCodec};
use stacks_common::util::hash::Sha512Trunc256Sum;
use stacks_common::util::secp256k1::MessageSignature;

use libstackerdb::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone)]
pub struct WrbTxtRecordV1 {
    pub contract_id: QualifiedContractIdentifier,
    pub slot_metadata: SlotMetadata,
}

/// Information embedded in a wrbsite TXT record in a BNS zonefile
#[derive(Debug, PartialEq, Clone)]
pub enum WrbTxtRecord {
    V1(WrbTxtRecordV1),
}

impl WrbTxtRecordV1 {
    pub fn new(contract_id: QualifiedContractIdentifier, slot_metadata: SlotMetadata) -> Self {
        Self {
            contract_id,
            slot_metadata,
        }
    }

    pub fn serialize_slot_metadata<W: Write>(&self, fd: &mut W) -> Result<(), CodecError> {
        write_next(fd, &self.slot_metadata.slot_id)?;
        write_next(fd, &self.slot_metadata.slot_version)?;
        write_next(fd, &self.slot_metadata.data_hash)?;
        write_next(fd, &self.slot_metadata.signature)?;
        Ok(())
    }

    pub fn serialize_contract_id<W: Write>(&self, fd: &mut W) -> Result<(), CodecError> {
        let principal = PrincipalData::Contract(self.contract_id.clone());
        write_next(fd, &principal)?;
        Ok(())
    }

    pub fn deserialize_slot_metadata<R: Read>(fd: &mut R) -> Result<SlotMetadata, CodecError> {
        let slot_id: u32 = read_next(fd)?;
        let slot_version: u32 = read_next(fd)?;
        let data_hash: Sha512Trunc256Sum = read_next(fd)?;
        let signature: MessageSignature = read_next(fd)?;
        Ok(SlotMetadata {
            slot_id,
            slot_version,
            data_hash,
            signature,
        })
    }

    pub fn deserialize_contract_id<R: Read>(
        fd: &mut R,
    ) -> Result<QualifiedContractIdentifier, CodecError> {
        let principal: PrincipalData = read_next(fd)?;
        if let PrincipalData::Contract(c) = principal {
            Ok(c)
        } else {
            Err(CodecError::DeserializeError(
                "Not a contract principal".into(),
            ))
        }
    }
}

impl StacksMessageCodec for WrbTxtRecordV1 {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), CodecError> {
        self.serialize_contract_id(fd)?;
        self.serialize_slot_metadata(fd)?;
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<Self, CodecError> {
        let contract_id = Self::deserialize_contract_id(fd)?;
        let slot_metadata = Self::deserialize_slot_metadata(fd)?;
        Ok(WrbTxtRecordV1 {
            contract_id,
            slot_metadata,
        })
    }
}

impl StacksMessageCodec for WrbTxtRecord {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), CodecError> {
        match self {
            Self::V1(payload) => {
                write_next(fd, &1u8)?;
                write_next(fd, payload)?;
            }
        }
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<Self, CodecError> {
        let version: u8 = read_next(fd)?;
        match version {
            1u8 => {
                let payload: WrbTxtRecordV1 = read_next(fd)?;
                Ok(Self::V1(payload))
            }
            _ => Err(CodecError::DeserializeError(format!(
                "Unsupported version {}",
                &version
            ))),
        }
    }
}

impl TryFrom<ZonefileResourceRecord> for WrbTxtRecord {
    type Error = runner::Error;
    fn try_from(mut rr: ZonefileResourceRecord) -> Result<WrbTxtRecord, Self::Error> {
        if rr.rr_name != "wrb" {
            return Err(Self::Error::Deserialize(
                "Resource record name is not 'wrb'".into(),
            ));
        }
        if rr.rr_type != "TXT" {
            return Err(Self::Error::Deserialize(
                "Resource record type is not 'TXT'".into(),
            ));
        }
        if rr.rr_class != "IN" {
            return Err(Self::Error::Deserialize(
                "Resource record class is not 'IN'".into(),
            ));
        }

        // de-quote
        if rr.rr_payload.starts_with("\"") && rr.rr_payload.ends_with("\"") {
            let Some(rr_payload) = rr.rr_payload.strip_prefix("\"").map(|s| s.to_string()) else {
                return Err(Self::Error::Deserialize(
                    "Failed to strip leading '\"'".into(),
                ));
            };
            let Some(mut rr_payload) = rr_payload.strip_suffix("\"").map(|s| s.to_string()) else {
                return Err(Self::Error::Deserialize(
                    "Failed to strip trailing '\"'".into(),
                ));
            };
            rr_payload = rr_payload.replace("\\\"", "\"");
            rr.rr_payload = rr_payload;
        }

        // extract bytes
        let bytes = Base64::decode_vec(&rr.rr_payload).map_err(|e| {
            Self::Error::Deserialize(format!("Failed to decode base64 TXT: {:?}", &e))
        })?;

        Ok(WrbTxtRecord::consensus_deserialize(&mut &bytes[..])
            .map_err(|_| Self::Error::Deserialize("Failed to decode WrbTxtRecord".into()))?)
    }
}

impl TryFrom<WrbTxtRecord> for ZonefileResourceRecord {
    type Error = runner::Error;
    fn try_from(wrb_txt: WrbTxtRecord) -> Result<Self, Self::Error> {
        let bytes = wrb_txt.serialize_to_vec();
        let bytes_b64 = Base64::encode_string(&bytes);
        if bytes_b64.len() > 256 {
            return Err(Self::Error::Serialize(format!(
                "Wrb TXT payload is too long: {} bytes",
                bytes_b64.len()
            )));
        }

        Ok(Self {
            rr_name: "wrb".into(),
            rr_ttl: None,
            rr_class: "IN".into(),
            rr_type: "TXT".into(),
            rr_payload: bytes_b64,
        })
    }
}

impl From<WrbTxtRecordV1> for WrbTxtRecord {
    fn from(wrb_rec: WrbTxtRecordV1) -> WrbTxtRecord {
        WrbTxtRecord::V1(wrb_rec)
    }
}

impl ToString for ZonefileResourceRecord {
    fn to_string(&self) -> String {
        let payload_quoted = Self::escape_string(&self.rr_payload).unwrap_or("\"\"".to_string());
        format!(
            "{}\t{}\t{}\t{}\t{}",
            &self.rr_name,
            &self
                .rr_ttl
                .as_ref()
                .map(|ttl| format!("{}", &ttl))
                .unwrap_or("".to_string()),
            &self.rr_class,
            &self.rr_type,
            &payload_quoted
        )
    }
}

impl WrbTxtRecord {
    pub fn new(contract_id: QualifiedContractIdentifier, slot_metadata: SlotMetadata) -> Self {
        Self::V1(WrbTxtRecordV1 {
            contract_id,
            slot_metadata,
        })
    }
}

/// Barebones zonefile resource record.
#[derive(Debug, PartialEq, Clone)]
pub struct ZonefileResourceRecord {
    /// the record name
    pub rr_name: String,
    /// record ttl
    pub rr_ttl: Option<u64>,
    /// the record class (IN, etc)
    pub rr_class: String,
    /// the record type (A, AAAA, MX, TXT, etc)
    pub rr_type: String,
    /// the record text (unescaped)
    pub rr_payload: String,
}

impl ZonefileResourceRecord {
    pub fn escape_string(s: &str) -> Option<String> {
        if !s.is_ascii() {
            return None;
        }
        let mut s = s.replace("\\", "\\\\");
        s = s.replace("\"", "\\\"");
        Some(format!("\"{}\"", &s))
    }

    pub fn unescape_string(s: &str) -> Option<String> {
        if !s.is_ascii() {
            return None;
        }
        let mut s = if s.starts_with("\"") && s.ends_with("\"") {
            let Some(s) = s.strip_prefix("\"").map(|s| s.to_string()) else {
                return None;
            };
            let Some(s) = s.strip_suffix("\"").map(|s| s.to_string()) else {
                return None;
            };
            s
        } else {
            s.to_string()
        };

        s = s.replace("\\\"", "\"");
        s = s.replace("\\\\", "\\");
        Some(s)
    }
}

impl Runner {
    /// Decode a zonefile into resource records.
    /// Limitations:
    /// * Only parses RRs. Not SOAs, $ORIGIN, or the like.
    /// * Each RR must be on a single line.
    /// * No validation is done on RR contents, other than ensuring TTL is an integer if given and
    /// that the class is a 2-byte string.
    /// * All remaining fields are concatenated. Spaces between them are dropped.
    pub fn decode_zonefile_records(
        zonefile: Vec<u8>,
    ) -> Result<Vec<ZonefileResourceRecord>, Error> {
        let zonefile_str = String::from_utf8(zonefile)
            .map_err(|e| Error::Deserialize(format!("Zonefile is not a UTF-8 string: {:?}", &e)))?;

        if !zonefile_str.is_ascii() {
            return Err(Error::Deserialize("Zonefile is not an ASCII string".into()));
        }

        let mut recs = vec![];
        let lines = zonefile_str.split("\n");
        for raw_line in lines {
            if raw_line.len() == 0 {
                continue;
            }
            let line = raw_line.trim();
            let tokens = line.split(&[' ', '\t']);

            let mut parts = Vec::with_capacity(4);
            let mut rr_ttl = None;
            let mut bad_line = false;
            for tok in tokens {
                if tok.len() == 0 {
                    continue;
                }

                if parts.len() == 1 {
                    // maybe TTL?
                    if let Ok(ttl) = tok.parse::<u64>() {
                        rr_ttl = Some(ttl);
                        continue;
                    }
                    // must be a class (2 bytes)
                    else if tok.len() != 2 {
                        // invalid class
                        bad_line = true;
                        break;
                    }
                }
                parts.push(tok.to_string());
            }

            if parts.len() < 4 || bad_line {
                // malformed
                continue;
            }

            let rr_payload = ZonefileResourceRecord::unescape_string(
                &parts.pop().expect("Unreachable -- checked length"),
            )
            .unwrap_or("".to_string());
            let rr_type = parts.pop().expect("Unreachable -- checked length");
            let rr_class = parts.pop().expect("Unreachable -- checked length");
            let rr_name = parts.pop().expect("Unreachable -- checked length");

            recs.push(ZonefileResourceRecord {
                rr_name,
                rr_ttl,
                rr_class,
                rr_type,
                rr_payload,
            });
        }
        Ok(recs)
    }

    /// Load a wrbsite, given the decoded wrb txt record.
    /// `wrbrec.slot_metadata` must have been authenticated.
    pub fn wrbsite_load_from_zonefile_rec(
        wrbrec: &WrbTxtRecordV1,
        replica_stackerdb_client: &mut dyn StackerDBClient,
    ) -> Result<Option<Vec<u8>>, Error> {
        let mut chunks = replica_stackerdb_client.get_chunks(&[(
            wrbrec.slot_metadata.slot_id,
            wrbrec.slot_metadata.slot_version,
        )])?;
        if chunks.len() != 1 {
            return Err(Error::Storage(format!(
                "Failed to get StackerDB chunk for site {}[{}.{}]: did not get any slots",
                &wrbrec.contract_id,
                wrbrec.slot_metadata.slot_id,
                wrbrec.slot_metadata.slot_version
            )));
        }

        let Some(Some(chunk_bytes)) = chunks.pop() else {
            return Err(Error::Storage(format!(
                "Failed to get StackerDB chunk for site {}[{}.{}]: no slot data returned",
                &wrbrec.contract_id,
                wrbrec.slot_metadata.slot_id,
                wrbrec.slot_metadata.slot_version
            )));
        };

        // authenticate
        let chunk_hash = Sha512Trunc256Sum::from_data(&chunk_bytes);
        if chunk_hash != wrbrec.slot_metadata.data_hash {
            return Err(Error::Storage(format!(
                "Site hash mismatch for site {}[{}.{}]: {} != {}",
                &wrbrec.contract_id,
                wrbrec.slot_metadata.slot_id,
                wrbrec.slot_metadata.slot_version,
                &wrbrec.slot_metadata.data_hash,
                &chunk_hash
            )));
        }

        Ok(Some(chunk_bytes))
    }

    /// Load a wrbsite, given the zonefile of a BNS name
    pub fn wrbsite_load_from_zonefile<F, G>(
        &mut self,
        zonefile: Vec<u8>,
        mut home_connector: F,
        mut replica_connector: G,
    ) -> Result<Option<(Vec<u8>, u32)>, Error>
    where
        F: FnMut(
            &QualifiedContractIdentifier,
            &SocketAddr,
        ) -> Result<Box<dyn StackerDBClient>, Error>,
        G: FnMut(
            &QualifiedContractIdentifier,
            &SocketAddr,
        ) -> Result<Box<dyn StackerDBClient>, Error>,
    {
        let Some(home_node_addr) = self.resolve_node()? else {
            return Err(Error::NotConnected);
        };
        let recs = Self::decode_zonefile_records(zonefile)?;

        let mut error_reasons = vec![];
        for (i, rec) in recs.into_iter().enumerate() {
            let rec_txt = rec.to_string();
            if rec.rr_name.as_str() != "wrb" {
                wrb_debug!("RR class is not 'wrb': '{}'", &rec_txt);
                error_reasons.push(format!("Invalid name in record {} of '{}'", i, &rec_txt));
                continue;
            }
            if rec.rr_class.as_str() != "IN" {
                wrb_debug!("RR class is not 'IN': '{}'", &rec_txt);
                error_reasons.push(format!("Invalid class in record {} of '{}'", i, &rec_txt));
                continue;
            }
            if rec.rr_type.as_str() != "TXT" {
                wrb_debug!("RR type is not 'TXT': '{}'", &rec_txt);
                error_reasons.push(format!("Invalid type in record {} of '{}'", i, &rec_txt));
                continue;
            }
            let Ok(wrbrec) = WrbTxtRecord::try_from(rec)
                .inspect_err(|e| wrb_warn!("Could not convert RR to WRB TXT record: {:?}", &e))
            else {
                wrb_debug!(
                    "Could not convert parsed record into WrbTxtRecord: '{}'",
                    &rec_txt
                );
                error_reasons.push(format!("Invalid payload in record {} '{}'", i, &rec_txt));
                continue;
            };

            let WrbTxtRecord::V1(wrbrec) = wrbrec;

            // query this replica's signers from the home node
            let mut home_client = home_connector(&wrbrec.contract_id, &home_node_addr)?;
            let signers = home_client.get_signers()?;

            // make sure the wrb txt record is consistent with the current signers
            if wrbrec.slot_metadata.slot_id >= u32::try_from(signers.len()).unwrap_or(u32::MAX) {
                wrb_info!(
                    "WRB record is at slot {}, which exceeds the number of signers ({})",
                    wrbrec.slot_metadata.slot_id,
                    signers.len()
                );
                error_reasons.push(format!(
                    "Slot {} exceeds number of signers ({}) in record {} '{}'",
                    wrbrec.slot_metadata.slot_id,
                    signers.len(),
                    i,
                    &rec_txt
                ));
                continue;
            }

            let Some(signer_addr) =
                signers.get(usize::try_from(wrbrec.slot_metadata.slot_id).unwrap_or(usize::MAX))
            else {
                wrb_info!(
                    "No such WRB signer for slot {}",
                    wrbrec.slot_metadata.slot_id
                );
                error_reasons.push(format!(
                    "No such signer for slot {} in record {} '{}'",
                    wrbrec.slot_metadata.slot_id, i, &rec_txt
                ));
                continue;
            };

            if let Err(e) = wrbrec.slot_metadata.verify(signer_addr) {
                wrb_warn!(
                    "Failed to authenticate slot {} with {}: {:?}",
                    wrbrec.slot_metadata.slot_id,
                    signer_addr,
                    &e
                );
                error_reasons.push(format!(
                    "Failed to authenticate slot {} with {} in record {} '{}'",
                    wrbrec.slot_metadata.slot_id, &signer_addr, i, &rec_txt
                ));
                continue;
            }

            // find nodes that replicate this stackerdb
            let replicas = home_client.find_replicas()?;
            if replicas.len() == 0 {
                wrb_warn!("No replicas found for StackerDB {}", &wrbrec.contract_id);
                error_reasons.push(format!(
                    "No replicas found for StackerDB {} in record {} '{}'",
                    &wrbrec.contract_id, i, &rec_txt
                ));
                continue;
            }
            for replica_addr in replicas.iter() {
                let Ok(mut replica_client) = replica_connector(&wrbrec.contract_id, replica_addr)
                    .inspect_err(|e| {
                        wrb_warn!(
                            "Failed to connect to replica {} of {}: {:?}",
                            replica_addr,
                            &wrbrec.contract_id,
                            &e
                        );
                        error_reasons.push(format!(
                            "Failed to connect to replica {} of {} in record {} '{}': {:?}",
                            replica_addr, &wrbrec.contract_id, i, &rec_txt, &e
                        ));
                    })
                else {
                    continue;
                };

                match Self::wrbsite_load_from_zonefile_rec(&wrbrec, &mut *replica_client) {
                    Ok(Some(wrbsite_bytes)) => {
                        return Ok(Some((wrbsite_bytes, wrbrec.slot_metadata.slot_version)));
                    }
                    Ok(None) => {
                        // not found
                        wrb_debug!("Skip WRB record {}", &rec_txt);
                        error_reasons.push(format!("Failed to load wrbsite from zonefile record {} '{}' for replica {} at {}", i, &rec_txt, &wrbrec.contract_id, replica_addr));
                        continue;
                    }
                    Err(e) => {
                        wrb_warn!(
                            "Failed to load WRB site for StackerDB {} from {}: {:?}",
                            &wrbrec.contract_id,
                            replica_addr,
                            &e
                        );
                        error_reasons.push(format!("Failed to load wrbsite from zonefile record {} '{}' for replica {} at {}: {:?}", i, &rec_txt, &wrbrec.contract_id, replica_addr, &e));
                        continue;
                    }
                }
            }
        }
        return Err(Error::FailedToRun(
            "Failed to resolve WRB site".into(),
            error_reasons,
        ));
    }

    /// Load a wrbsite, given the BNS name, resolver, and StackerDB connectors
    pub fn wrbsite_load_ext<F, G>(
        &mut self,
        bns_resolver: &mut dyn BNSResolver,
        name: &str,
        namespace: &str,
        home_connector: F,
        replica_connector: G,
    ) -> Result<Option<(Vec<u8>, u32)>, Error>
    where
        F: FnMut(
            &QualifiedContractIdentifier,
            &SocketAddr,
        ) -> Result<Box<dyn StackerDBClient>, Error>,
        G: FnMut(
            &QualifiedContractIdentifier,
            &SocketAddr,
        ) -> Result<Box<dyn StackerDBClient>, Error>,
    {
        let bns_rec = match bns_resolver.lookup(self, name, namespace) {
            Ok(Ok(rec)) => rec,
            Ok(Err(bns_e)) => {
                wrb_warn!(
                    "Failed to resolve '{}.{}' to zonefile due to contract error: {:?}",
                    name,
                    namespace,
                    &bns_e
                );
                return Err(Error::FailedToRun(
                    "Failed to resolve name to zonefile".into(),
                    vec![format!(
                        "Failed to resolve '{}.{}' to zonefile due to contract error: {:?}",
                        name, namespace, &bns_e
                    )],
                ));
            }
            Err(e) => {
                wrb_error!(
                    "Failed to resolve '{}.{}' to zonefile: {:?}",
                    name,
                    namespace,
                    &e
                );
                return Err(Error::FailedToRun(
                    "Failed to resolve '{}.{}' to zonefile due to error".into(),
                    vec![format!("Lookup failed: {:?}", &e)],
                ));
            }
        };
        let Some(zonefile) = bns_rec.zonefile else {
            wrb_warn!("Name '{}.{}' has no zonefile", name, namespace);
            return Err(Error::FailedToRun("Name has no zonefile".into(), vec![]));
        };

        self.wrbsite_load_from_zonefile(zonefile, home_connector, replica_connector)
    }

    /// Home node connector
    pub fn home_node_connect(
        contract_id: &QualifiedContractIdentifier,
        node_addr: &SocketAddr,
    ) -> Result<Box<dyn StackerDBClient>, Error> {
        let session = StackerDBSession::new(node_addr.clone(), contract_id.clone());
        Ok(Box::new(session))
    }

    /// Replica node connector
    pub fn replica_node_connect(
        contract_id: &QualifiedContractIdentifier,
        home_node_addr: &SocketAddr,
        node_p2p_addr: &SocketAddr,
    ) -> Result<Box<dyn StackerDBClient>, Error> {
        let node_addr = Self::run_resolve_stackerdb_host(home_node_addr, node_p2p_addr)?;
        wrb_debug!(
            "wrbsite_load: resolved replica node {} to {}",
            node_p2p_addr,
            &node_addr
        );

        let session = StackerDBSession::new(node_addr, contract_id.clone());
        Ok(Box::new(session))
    }

    /// Helper method to load a wrbsite, given a BNS name and resolver.
    /// Returns Some((bytes, version)) on success
    /// Returns None if the name doesn't exist
    pub fn wrbsite_load(
        &mut self,
        bns_resolver: &mut dyn BNSResolver,
        name: &str,
        namespace: &str,
    ) -> Result<Option<(Vec<u8>, u32)>, Error> {
        let Some(home_node_addr) = self.resolve_node()? else {
            return Err(Error::NotConnected);
        };

        self.wrbsite_load_ext(
            bns_resolver,
            name,
            namespace,
            |contract_id: &QualifiedContractIdentifier, node_addr: &SocketAddr| {
                Runner::home_node_connect(contract_id, node_addr)
            },
            |contract_id: &QualifiedContractIdentifier, node_p2p_addr: &SocketAddr| {
                Runner::replica_node_connect(contract_id, &home_node_addr, node_p2p_addr)
            },
        )
    }
}
