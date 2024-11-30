// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2022 Stacks Open Internet Foundation
// Copyright (C) 2022-2024 Jude Nelson
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

//! TODO: DEPRICATED -- this is BNSv1

use std::collections::HashMap;
use std::convert::TryFrom;
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use std::io::Write;

use crate::core::Config;
use crate::runner::Error;
use crate::runner::Runner;

use clarity::vm::types::BufferLength;
use clarity::vm::types::PrincipalData;
use clarity::vm::types::ResponseData;
use clarity::vm::types::QualifiedContractIdentifier;
use clarity::vm::types::SequenceData;
use clarity::vm::types::Value;

use stacks_common::util::hash::Hash160;

#[derive(Debug, PartialEq, Clone)]
pub struct BNSNameRecord {
    pub zonefile_hash: Hash160,
    pub owner: PrincipalData,
    pub lease_started_at: u128,
    pub lease_ending_at: Option<u128>,
}

impl TryFrom<ResponseData> for BNSNameRecord {
    type Error = Error;
    fn try_from(v_ok: ResponseData) -> Result<Self, Error> {
        if !v_ok.committed {
            return Err(Error::Deserialize("Expected Ok-response".into()));
        }
        let Value::Tuple(data) = *v_ok.data else {
            return Err(Error::Deserialize("Expected tuple".into()));
        };
        let zonefile_hash_value = data
            .get("zonefile-hash")
            .map_err(|_| Error::Deserialize("Expected zonefile-hash".into()))?
            .clone();
        let owner_value = data
            .get("owner")
            .map_err(|_| Error::Deserialize("Expected owner".into()))?
            .clone();
        let lease_started_at_value = data
            .get("lease-started-at")
            .map_err(|_| Error::Deserialize("Expected lease-started-at".into()))?
            .clone();
        let lease_ending_at_value = data
            .get("lease-ending-at")
            .map_err(|_| Error::Deserialize("Expected lease-ending-at".into()))?
            .clone();

        let Value::Sequence(SequenceData::Buffer(zonefile_hash_bytes)) = zonefile_hash_value else {
            return Err(Error::Deserialize("Expected buff".into()));
        };
        if u32::from(zonefile_hash_bytes.len()?) != 20 {
            return Err(Error::Deserialize("Expected (buff 20)".into()));
        }

        let Value::Principal(owner) = owner_value else {
            return Err(Error::Deserialize("Expected principal".into()));
        };
        let Value::UInt(lease_started_at) = lease_started_at_value else {
            return Err(Error::Deserialize(
                "Expected uint for lease_started_at".into(),
            ));
        };
        let Value::Optional(lease_ending_at) = lease_ending_at_value else {
            return Err(Error::Deserialize(
                "Expected optional for lease_ending_at".into(),
            ));
        };
        let lease_ending_at = if let Some(lease_ending_at) = lease_ending_at.data {
            let Value::UInt(lease_ending_at) = *lease_ending_at else {
                return Err(Error::Deserialize(
                    "Expected uint for lease_ending_at".into(),
                ));
            };
            Some(lease_ending_at)
        } else {
            None
        };

        let mut zonefile_hash160_bytes = [0u8; 20];
        zonefile_hash160_bytes.copy_from_slice(&zonefile_hash_bytes.data);

        Ok(BNSNameRecord {
            zonefile_hash: Hash160(zonefile_hash160_bytes),
            owner,
            lease_started_at,
            lease_ending_at,
        })
    }
}

#[derive(Debug)]
pub enum BNSError {
    NameNotFound,
    NamespaceNotFound,
    NameGracePeriod,
    NameExpired
}

impl TryFrom<ResponseData> for BNSError {
    type Error = Error;
    fn try_from(v_err: ResponseData) -> Result<Self, Error> {
        let Value::UInt(errcode) = *v_err.data else {
            return Err(Error::Deserialize("Expected uint".into()));
        };
        match errcode {
            2013 => Ok(Self::NameNotFound),
            1005 => Ok(Self::NamespaceNotFound),
            2009 => Ok(Self::NameGracePeriod),
            2008 => Ok(Self::NameExpired),
            _ => Err(Error::Deserialize("Unrecognized error code".into()))
        }
    }
}

impl Runner {
    /// Look up a BNS name
    pub fn bns_lookup(&mut self, namespace: &str, name: &str) -> Result<Result<BNSNameRecord, BNSError>, Error> {
        let bns_contract = self.bns_contract_id.clone();
        let v = self.call_readonly(&bns_contract, "name-resolve", &[Value::string_ascii_from_bytes(namespace.as_bytes().to_vec())?, Value::string_ascii_from_bytes(name.as_bytes().to_vec())?])?;
        let Value::Response(v_res) = v else {
            return Err(Error::Deserialize("Expected response".into()));
        };
        if v_res.committed {
            return Ok(Ok(BNSNameRecord::try_from(v_res)?));
        }
        else {
            return Ok(Err(BNSError::try_from(v_res)?));
        }
    }
}
