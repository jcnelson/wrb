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
use crate::runner::Error;
use crate::runner::Runner;

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
    fn try_from(rr: ZonefileResourceRecord) -> Result<WrbTxtRecord, Self::Error> {
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
        if rr.rr_payload.len() == 0 {
            // should be unreachable
            return Err(Self::Error::Deserialize(
                "Resource record payload is missing".into(),
            ));
        }

        // extract bytes
        let bytes = Base64::decode_vec(&rr.rr_payload[0]).map_err(|e| {
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
            rr_payload: vec![bytes_b64],
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
        let base_string = format!(
            "{}\t{}\t{}\t{}\t",
            &self.rr_name,
            &self
                .rr_ttl
                .as_ref()
                .map(|ttl| format!("{}", &ttl))
                .unwrap_or("".to_string()),
            &self.rr_class,
            &self.rr_type
        );

        let quoted_payload = self
            .rr_payload
            .iter()
            .map(|s| format!("\"{}\"", s.replace("\"", "\\\"")))
            .fold(base_string, |mut rr_str, payload_str| {
                rr_str.push_str(&payload_str);
                rr_str.push_str(" ");
                rr_str
            });

        quoted_payload
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
    /// the record text (not decoded)
    pub rr_payload: Vec<String>,
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

            let mut first_parts = Vec::with_capacity(4);
            let mut rr_ttl = None;
            let mut rr_payload = vec![];
            let mut bad_line = false;
            for tok in tokens {
                if tok.len() == 0 {
                    continue;
                }

                if first_parts.len() == 1 {
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
                } else if first_parts.len() == 3 {
                    rr_payload.push(tok.to_string());
                    continue;
                }

                first_parts.push(tok.to_string());
            }

            if first_parts.len() < 3 || bad_line {
                // malformed
                continue;
            }

            if rr_payload.len() == 0 {
                // malformed
                continue;
            }

            let rr_type = first_parts.pop().expect("Unreachable -- checked length");
            let rr_class = first_parts.pop().expect("Unreachable -- checked length");
            let rr_name = first_parts.pop().expect("Unreachable -- checked length");

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
        &mut self,
        wrbrec: WrbTxtRecordV1,
    ) -> Result<Option<Vec<u8>>, Error> {
        let Some(node_addr) = self.resolve_node()? else {
            return Err(Error::NotConnected);
        };
        Self::run_get_stackerdb_chunk(
            &node_addr,
            &wrbrec.contract_id,
            wrbrec.slot_metadata.slot_id,
            wrbrec.slot_metadata.slot_version,
        )
    }

    /// Load a wrbsite, given the zonefile of a BNS name
    pub fn wrbsite_load_from_zonefile(
        &mut self,
        zonefile: Vec<u8>,
    ) -> Result<Option<Vec<u8>>, Error> {
        let recs = Self::decode_zonefile_records(zonefile)?;

        for rec in recs.into_iter() {
            let rec_txt = rec.to_string();
            if rec.rr_name.as_str() != "wrb" {
                continue;
            }
            if rec.rr_class.as_str() != "IN" {
                continue;
            }
            if rec.rr_type.as_str() != "TXT" {
                continue;
            }
            let Ok(wrbrec) = WrbTxtRecord::try_from(rec) else {
                continue;
            };

            let WrbTxtRecord::V1(wrbrec) = wrbrec;

            // query this replica's signers
            let signers = self.get_stackerdb_signers(&wrbrec.contract_id)?;

            // make sure the wrb txt record is consistent with the current signers
            if wrbrec.slot_metadata.slot_id >= u32::try_from(signers.len()).unwrap_or(u32::MAX) {
                wrb_info!(
                    "WRB record is at slot {}, which exceeds the number of signers ({})",
                    wrbrec.slot_metadata.slot_id,
                    signers.len()
                );
                continue;
            }

            let Some(signer_addr) =
                signers.get(usize::try_from(wrbrec.slot_metadata.slot_id).unwrap_or(usize::MAX))
            else {
                wrb_info!(
                    "No such WRB signer for slot {}",
                    wrbrec.slot_metadata.slot_id
                );
                continue;
            };

            if let Err(e) = wrbrec.slot_metadata.verify(signer_addr) {
                wrb_warn!(
                    "Failed to authenticate slot {} with {}: {:?}",
                    wrbrec.slot_metadata.slot_id,
                    signer_addr,
                    &e
                );
                continue;
            }

            match self.wrbsite_load_from_zonefile_rec(wrbrec) {
                Ok(Some(wrbsite_bytes)) => {
                    return Ok(Some(wrbsite_bytes));
                }
                Ok(None) => {
                    // not found
                    wrb_debug!("Skip WRB record {}", &rec_txt);
                    continue;
                }
                Err(e) => {
                    wrb_warn!("Failed to load WRB site: {:?}", &e);
                    continue;
                }
            }
        }
        return Err(Error::FailedToRun("Failed to resolve WRB site".into()));
    }

    /// Load a wrbsite, given the BNS name
    pub fn wrbsite_load(
        &mut self,
        bns_resolver: &mut dyn BNSResolver,
        name: &str,
        namespace: &str,
    ) -> Result<Option<Vec<u8>>, Error> {
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
                ));
            }
            Err(e) => {
                wrb_error!(
                    "Failed to resolve '{}.{}' to zonefile: {:?}",
                    name,
                    namespace,
                    &e
                );
                return Err(e);
            }
        };
        let Some(zonefile) = bns_rec.zonefile else {
            wrb_warn!("Name '{}.{}' has no zonefile", name, namespace);
            return Err(Error::FailedToRun("Name has no zonefile".into()));
        };

        self.wrbsite_load_from_zonefile(zonefile)
    }
}
