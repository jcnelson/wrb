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

#[derive(Debug, PartialEq, Clone)]
pub struct BNSNameRecord {
    pub zonefile: Option<Vec<u8>>,
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

#[derive(Debug)]
pub enum BNSError {
    NameNotFound,
    NamespaceNotFound,
    NameGracePeriod,
    NameExpired,
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
            _ => Err(Error::Deserialize("Unrecognized error code".into())),
        }
    }
}

impl Runner {
    /// Look up a BNS name.
    /// Must be from the `zonefile-resolver` contract in BNSv2
    pub fn bns_lookup(
        &mut self,
        namespace: &str,
        name: &str,
    ) -> Result<Result<BNSNameRecord, BNSError>, Error> {
        let bns_contract = self.bns_contract_id.clone();
        let v = self.call_readonly(
            &bns_contract,
            "name-resolve",
            &[
                Value::string_ascii_from_bytes(namespace.as_bytes().to_vec())?,
                Value::string_ascii_from_bytes(name.as_bytes().to_vec())?,
            ],
        )?;
        let Value::Response(v_res) = v else {
            return Err(Error::Deserialize("Expected response".into()));
        };
        if v_res.committed {
            return Ok(Ok(BNSNameRecord::try_from(v_res)?));
        } else {
            return Ok(Err(BNSError::try_from(v_res)?));
        }
    }
}
