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
use clarity::vm::types::QualifiedContractIdentifier;
use clarity::vm::types::ResponseData;
use clarity::vm::types::SequenceData;
use clarity::vm::types::Value;

use stacks_common::util::hash::Hash160;

use serde;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone)]
pub struct BNSNameRecord {
    pub zonefile: Option<Vec<u8>>,
}

impl BNSNameRecord {
    pub fn empty() -> Self {
        Self { zonefile: None }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct BNSNameOwner {
    pub renewal: u128,
    pub owner: PrincipalData,
}

impl TryFrom<ResponseData> for BNSNameOwner {
    type Error = Error;
    fn try_from(v_ok: ResponseData) -> Result<Self, Self::Error> {
        if !v_ok.committed {
            return Err(Error::Deserialize("Expected Ok-response".into()));
        }

        let Value::Tuple(tuple) = *v_ok.data else {
            return Err(Error::Deserialize("BNS name info is not a tuple".into()));
        };

        let renewal = tuple
            .get("renewal")
            .cloned()
            .or_else(|_| {
                Err(Error::Deserialize(
                    "BNS name owner tuple is missing `renewal-height`".into(),
                ))
            })?
            .expect_u128()
            .or_else(|_| {
                Err(Error::Deserialize(
                    "BNS `renewal` value is not a u128".into(),
                ))
            })?;

        let owner = tuple
            .get("owner")
            .cloned()
            .or_else(|_| {
                Err(Error::Deserialize(
                    "BNS name owner tuple is missing `owner`".into(),
                ))
            })?
            .expect_principal()
            .or_else(|_| {
                Err(Error::Deserialize(
                    "BNS `owner` value is not a principal".into(),
                ))
            })?;

        Ok(Self { renewal, owner })
    }
}

impl TryFrom<ResponseData> for BNSNameRecord {
    type Error = Error;
    fn try_from(v_ok: ResponseData) -> Result<Self, Error> {
        if !v_ok.committed {
            return Err(Error::Deserialize("Expected Ok-response".into()));
        }
        let Value::Optional(zonefile_opt) = *v_ok.data else {
            return Err(Error::Deserialize("Expected optional".into()));
        };
        let zonefile = if let Some(zonefile_value) = zonefile_opt.data {
            let Value::Sequence(SequenceData::Buffer(zonefile_bytes)) = *zonefile_value else {
                return Err(Error::Deserialize("Expected buff".into()));
            };
            if u32::from(zonefile_bytes.len()?) > 8192 {
                return Err(Error::Deserialize("Expected (buff 8192)".into()));
            }
            Some(zonefile_bytes.data)
        } else {
            None
        };

        Ok(BNSNameRecord { zonefile })
    }
}

#[derive(Debug, Clone)]
pub enum BNSError {
    NoZonefileFound,
    NameNotFound,
    NamespaceNotFound,
    NameRevoked,
}

impl BNSError {
    pub fn name_exists(&self) -> bool {
        matches!(self, BNSError::NoZonefileFound)
    }
}

impl TryFrom<ResponseData> for BNSError {
    type Error = Error;
    fn try_from(v_err: ResponseData) -> Result<Self, Error> {
        let Value::UInt(errcode) = *v_err.data else {
            return Err(Error::Deserialize("Expected uint".into()));
        };
        match errcode {
            101 => Ok(Self::NoZonefileFound),
            102 => Ok(Self::NameNotFound),
            103 => Ok(Self::NamespaceNotFound),
            104 => Ok(Self::NameRevoked),
            x => Err(Error::Deserialize(format!("Unrecognized error code {}", x))),
        }
    }
}

pub trait BNSResolver {
    fn lookup(
        &mut self,
        runner: &mut Runner,
        name: &str,
        namespace: &str,
    ) -> Result<Result<BNSNameRecord, BNSError>, Error>;

    fn get_owner(
        &mut self,
        runner: &mut Runner,
        name: &str,
        namespace: &str,
    ) -> Result<Option<BNSNameOwner>, Error>;

    fn get_price(
        &mut self,
        runner: &mut Runner,
        name: &str,
        namespace: &str,
    ) -> Result<Option<u128>, Error>;
}

pub struct NodeBNSResolver {}

impl NodeBNSResolver {
    pub fn new() -> Self {
        Self {}
    }
}

impl BNSResolver for NodeBNSResolver {
    fn lookup(
        &mut self,
        runner: &mut Runner,
        name: &str,
        namespace: &str,
    ) -> Result<Result<BNSNameRecord, BNSError>, Error> {
        runner.bns_lookup(name, namespace)
    }

    fn get_owner(
        &mut self,
        runner: &mut Runner,
        name: &str,
        namespace: &str,
    ) -> Result<Option<BNSNameOwner>, Error> {
        runner.bns_get_name_owner(name, namespace)
    }

    fn get_price(
        &mut self,
        runner: &mut Runner,
        name: &str,
        namespace: &str,
    ) -> Result<Option<u128>, Error> {
        runner.bns_get_name_price(name, namespace)
    }
}

impl Runner {
    /// Look up a BNS name.
    /// Must be from the `zonefile-resolver` contract in BNSv2
    pub fn bns_lookup(
        &mut self,
        name: &str,
        namespace: &str,
    ) -> Result<Result<BNSNameRecord, BNSError>, Error> {
        let zonefile_contract = self.zonefile_contract_id.clone();
        let v = self.call_readonly(
            &zonefile_contract,
            "resolve-name",
            &[
                Value::buff_from(name.as_bytes().to_vec())?,
                Value::buff_from(namespace.as_bytes().to_vec())?,
            ],
        )?;
        let Value::Response(v_res) = v else {
            return Err(Error::Deserialize("Expected response".into()));
        };
        if v_res.committed {
            return Ok(Ok(BNSNameRecord::try_from(v_res)?));
        } else {
            let bns_err = BNSError::try_from(v_res)?;
            if let BNSError::NoZonefileFound = bns_err {
                return Ok(Ok(BNSNameRecord::empty()));
            }
            return Ok(Err(bns_err));
        }
    }

    /// Determine BNS name owner
    pub fn bns_get_name_owner(
        &mut self,
        name: &str,
        namespace: &str,
    ) -> Result<Option<BNSNameOwner>, Error> {
        let bns_contract = self.bns_contract_id.clone();
        let v = self.call_readonly(
            &bns_contract,
            "can-resolve-name",
            &[
                Value::buff_from(namespace.as_bytes().to_vec())?,
                Value::buff_from(name.as_bytes().to_vec())?,
            ],
        )?;
        let Value::Response(v_res) = v else {
            return Err(Error::Deserialize("Expected response".into()));
        };
        if v_res.committed {
            return Ok(Some(BNSNameOwner::try_from(v_res)?));
        } else {
            // name or namespace not found
            return Ok(None);
        }
    }

    /// Determine BNS name price
    pub fn bns_get_name_price(
        &mut self,
        name: &str,
        namespace: &str,
    ) -> Result<Option<u128>, Error> {
        let bns_contract = self.bns_contract_id.clone();
        let v = self.call_readonly(
            &bns_contract,
            "get-name-price",
            &[
                Value::buff_from(namespace.as_bytes().to_vec())?,
                Value::buff_from(name.as_bytes().to_vec())?,
            ],
        )?;
        let Value::Response(v_res) = v else {
            return Err(Error::Deserialize("Expected response".into()));
        };
        if v_res.committed {
            let Value::Response(price_res) = *v_res.data else {
                return Err(Error::Deserialize("Price-res is not a response".into()));
            };
            if !price_res.committed {
                return Err(Error::Deserialize(
                    "Error getting BNS response for price".into(),
                ));
            }
            let Value::UInt(price) = *price_res.data else {
                return Err(Error::Deserialize("Proce is not a u128".into()));
            };
            return Ok(Some(price));
        } else {
            // namespace not found
            return Ok(None);
        }
    }
}
