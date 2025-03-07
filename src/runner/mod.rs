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
use std::convert::TryFrom;
use std::error;
use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::net::ToSocketAddrs;

use crate::runner::http::run_http_request;

use clarity::vm::types::QualifiedContractIdentifier;
use clarity::vm::Value;

use stacks_common::types::chainstate::BurnchainHeaderHash;
use stacks_common::types::chainstate::SortitionId;
use stacks_common::types::chainstate::StacksAddress;
use stacks_common::types::chainstate::{BlockHeaderHash, ConsensusHash, StacksBlockId};
use stacks_common::types::net::PeerAddress;
use stacks_common::types::StacksPublicKeyBuffer;
use stacks_common::util::hash::{hex_bytes, Hash160, Sha256Sum};
use stacks_common::util::HexError;

use serde::Deserialize;
use serde::Serialize;

use clarity::vm::errors::Error as clarity_error;
use clarity::vm::errors::InterpreterError as clarity_interpreter_error;

use crate::net::NeighborAddress;

pub mod bns;
pub mod http;
pub mod process;
pub mod site;
pub mod stackerdb;
pub mod tx;

#[cfg(test)]
pub mod tests;

/// The response to GET /v2/info, omitting things like the anchor block and affirmation maps (since
/// we don't have the structs for them available in stacks_common).
/// Cribbed from the Stacks blockchain (https://github.com/stacks-network/stacks-core)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RPCPeerInfoData {
    pub peer_version: u32,
    pub pox_consensus: ConsensusHash,
    pub burn_block_height: u64,
    pub stable_pox_consensus: ConsensusHash,
    pub stable_burn_block_height: u64,
    pub server_version: String,
    pub network_id: u32,
    pub parent_network_id: u32,
    pub stacks_tip_height: u64,
    pub stacks_tip: BlockHeaderHash,
    pub stacks_tip_consensus_hash: ConsensusHash,
    pub genesis_chainstate_hash: Sha256Sum,
    pub unanchored_tip: Option<StacksBlockId>,
    pub unanchored_seq: Option<u16>,
    pub exit_at_block_height: Option<u64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_public_key: Option<StacksPublicKeyBuffer>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_public_key_hash: Option<Hash160>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stackerdbs: Option<Vec<String>>,
}

/// Struct for sortition information returned via the GetSortition API call
#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub struct RPCSortitionInfo {
    /// The burnchain header hash of the block that triggered this event.
    #[serde(with = "prefix_hex")]
    pub burn_block_hash: BurnchainHeaderHash,
    /// The burn height of the block that triggered this event.
    pub burn_block_height: u64,
    /// The burn block time of the sortition
    pub burn_header_timestamp: u64,
    /// This sortition ID of the block that triggered this event. This incorporates
    ///  PoX forking information and the burn block hash to obtain an identifier that is
    ///  unique across PoX forks and burnchain forks.
    #[serde(with = "prefix_hex")]
    pub sortition_id: SortitionId,
    /// The parent of this burn block's Sortition ID
    #[serde(with = "prefix_hex")]
    pub parent_sortition_id: SortitionId,
    /// The consensus hash of the block that triggered this event. This incorporates
    ///  PoX forking information and burn op information to obtain an identifier that is
    ///  unique across PoX forks and burnchain forks.
    #[serde(with = "prefix_hex")]
    pub consensus_hash: ConsensusHash,
    /// Boolean indicating whether or not there was a succesful sortition (i.e. a winning
    ///  block or miner was chosen).
    ///
    /// This will *also* be true if this sortition corresponds to a shadow block.  This is because
    /// the signer does not distinguish between shadow blocks and blocks with sortitions, so until
    /// we can update the signer and this interface, we'll have to report the presence of a shadow
    /// block tenure in a way that the signer currently understands.
    pub was_sortition: bool,
    /// If sortition occurred, and the miner's VRF key registration
    ///  associated a nakamoto mining pubkey with their commit, this
    ///  will contain the Hash160 of that mining key.
    #[serde(with = "prefix_opt_hex")]
    pub miner_pk_hash160: Option<Hash160>,
    /// If sortition occurred, this will be the consensus hash of the burn block corresponding
    /// to the winning block commit's parent block ptr. In 3.x, this is the consensus hash of
    /// the tenure that this new burn block's miner will be building off of.
    #[serde(with = "prefix_opt_hex")]
    pub stacks_parent_ch: Option<ConsensusHash>,
    /// If sortition occurred, this will be the consensus hash of the most recent sortition before
    ///  this one.
    #[serde(with = "prefix_opt_hex")]
    pub last_sortition_ch: Option<ConsensusHash>,
    #[serde(with = "prefix_opt_hex")]
    /// In Stacks 2.x, this is the winning block.
    /// In Stacks 3.x, this is the first block of the parent tenure.
    pub committed_block_hash: Option<BlockHeaderHash>,
}

#[derive(Debug, Clone)]
pub enum Error {
    FailedToRun(String, Vec<String>),
    FailedToExecute(String, String),
    KilledBySignal(String),
    BadExit(i32),
    InvalidOutput(String),
    IO(String),
    Serialize(String),
    Deserialize(String),
    NotConnected,
    NotInitialized,
    NoFeeEstimate,
    MalformedRequest(String),
    MalformedResponse(String),
    HttpError(u32, HashMap<String, String>, usize),
    RPCError(String),
    Storage(String),
    Clarity(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::FailedToRun(ref cmd, ref reasons) => {
                write!(f, "Failed to run '{}'. Reasons: {:?}", cmd, reasons)
            }
            Error::FailedToExecute(ref cmd, ref ioe) => {
                write!(f, "Failed to run '{}': {}", cmd, ioe)
            }
            Error::KilledBySignal(ref cmd) => {
                write!(f, "Failed to run '{}': killed by signal", cmd)
            }
            Error::BadExit(ref es) => write!(f, "Command exited with status {}", es),
            Error::InvalidOutput(ref s) => write!(f, "Invalid command output: '{}'", s),
            Error::IO(ref e) => write!(f, "IO Error: {}", e),
            Error::Serialize(ref s) => write!(f, "Serialize error: {}", s),
            Error::Deserialize(ref s) => write!(f, "Deserialize error: {}", s),
            Error::NotConnected => write!(f, "Not connected"),
            Error::NotInitialized => write!(f, "System not initialized"),
            Error::NoFeeEstimate => write!(f, "No fee estimate at this time"),
            Error::MalformedRequest(ref s) => write!(f, "Malformed request: {}", s),
            Error::MalformedResponse(ref s) => write!(f, "Malformed response: {}", s),
            Error::HttpError(ref code, ref _headers, ref _body_offset) => {
                write!(f, "Bad HTTP code: {}", code)
            }
            Error::RPCError(ref msg) => write!(f, "RPC error: {}", msg),
            Error::Storage(ref msg) => write!(f, "Storage error: {}", msg),
            Error::Clarity(ref err) => write!(f, "Clarity error: {}", err),
        }
    }
}

impl error::Error for Error {
    fn cause(&self) -> Option<&dyn error::Error> {
        match *self {
            Error::FailedToRun(..) => None,
            Error::FailedToExecute(..) => None,
            Error::KilledBySignal(_) => None,
            Error::BadExit(_) => None,
            Error::InvalidOutput(_) => None,
            Error::IO(..) => None,
            Error::Serialize(_) => None,
            Error::Deserialize(_) => None,
            Error::NotConnected => None,
            Error::NotInitialized => None,
            Error::NoFeeEstimate => None,
            Error::MalformedRequest(_) => None,
            Error::MalformedResponse(_) => None,
            Error::HttpError(..) => None,
            Error::RPCError(_) => None,
            Error::Storage(_) => None,
            Error::Clarity(_) => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::IO(format!("{:?}", &e))
    }
}

impl From<clarity_error> for Error {
    fn from(e: clarity_error) -> Error {
        Error::Clarity(format!("{:?}", &e))
    }
}

impl From<clarity_interpreter_error> for Error {
    fn from(e: clarity_interpreter_error) -> Error {
        Error::Clarity(format!("InterpreterError: {:?}", &e))
    }
}

pub struct Runner {
    bns_contract_id: QualifiedContractIdentifier,
    zonefile_contract_id: QualifiedContractIdentifier,
    node_host: String,
    node_port: u16,
    node: Option<SocketAddr>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CallReadOnlyRequestBody {
    pub sender: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sponsor: Option<String>,
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallReadOnlyResponse {
    pub okay: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

impl Runner {
    pub fn resolve_node(&mut self) -> Result<Option<SocketAddr>, Error> {
        if self.node.is_none() {
            let mut addrs: Vec<_> = (self.node_host.as_str(), self.node_port)
                .to_socket_addrs()?
                .collect();
            return Ok(addrs.pop());
        }
        Ok(self.node.clone())
    }

    pub fn get_bns_contract_id(&self) -> QualifiedContractIdentifier {
        self.bns_contract_id.clone()
    }

    pub fn get_zonefile_contract_id(&self) -> QualifiedContractIdentifier {
        self.zonefile_contract_id.clone()
    }

    /// Run a read-only function call on the node, given a resolved socket address to the node
    pub fn run_call_readonly(
        node_addr: &SocketAddr,
        contract_id: &QualifiedContractIdentifier,
        function_name: &str,
        function_args: &[Value],
    ) -> Result<Value, Error> {
        let mut sock = TcpStream::connect(node_addr)?;

        let mut arguments = vec![];
        for arg in function_args.iter() {
            let v = arg.serialize_to_hex()?;
            arguments.push(v);
        }

        let payload = CallReadOnlyRequestBody {
            sender: format!("{}", &contract_id.issuer),
            sponsor: None,
            arguments,
        };
        let payload_json = serde_json::to_string(&payload)
            .map_err(|_| Error::RPCError("Could not serialize call-read-only request".into()))?;

        wrb_debug!(
            "call-readonly {} {}: {}",
            &contract_id,
            function_name,
            &payload_json
        );
        let bytes = run_http_request(
            &mut sock,
            node_addr,
            "POST",
            &format!(
                "/v2/contracts/call-read/{}/{}/{}",
                &contract_id.issuer, &contract_id.name, function_name
            ),
            Some("application/json"),
            payload_json.as_bytes(),
        )?;

        // try to convert into the response
        let response: CallReadOnlyResponse = serde_json::from_slice(&bytes).map_err(|_| {
            Error::Deserialize("Failed to decode call-read-only response bytes".into())
        })?;
        if !response.okay {
            return Err(Error::RPCError(format!(
                "reason: {}",
                &response.cause.unwrap_or("(no cause given)".into())
            )));
        }

        let Some(result) = response.result else {
            return Err(Error::RPCError("No result given".into()));
        };

        let result = result
            .strip_prefix("0x")
            .map(|s| s.to_string())
            .unwrap_or(result);
        let value = Value::try_deserialize_hex_untyped(&result).map_err(|_| {
            Error::Deserialize(format!(
                "Failed to decode hex string into clarity value: {}",
                &result
            ))
        })?;
        Ok(value)
    }

    /// Run a read-only function call on the node, using the resolved node.
    pub fn call_readonly(
        &mut self,
        contract_id: &QualifiedContractIdentifier,
        function_name: &str,
        function_args: &[Value],
    ) -> Result<Value, Error> {
        let Some(node_addr) = self.resolve_node()? else {
            return Err(Error::NotConnected);
        };
        Self::run_call_readonly(&node_addr, contract_id, function_name, function_args)
    }

    /// Get /v2/info
    pub fn run_get_info(node_addr: &SocketAddr) -> Result<RPCPeerInfoData, Error> {
        let mut sock = TcpStream::connect(node_addr)?;
        let bytes = run_http_request(&mut sock, node_addr, "GET", "/v2/info", None, &[])?;

        let response: RPCPeerInfoData = serde_json::from_slice(&bytes)
            .map_err(|_| Error::Deserialize("Failed to decode /v2/info response".into()))?;

        Ok(response)
    }

    /// Get /v3/sortitions/{:key}/{:value}
    pub fn run_get_sortition_info(
        node_addr: &SocketAddr,
        key: &str,
        value: &str,
    ) -> Result<Vec<RPCSortitionInfo>, Error> {
        let mut sock = TcpStream::connect(node_addr)?;
        let bytes = run_http_request(
            &mut sock,
            node_addr,
            "GET",
            &format!("/v3/sortitions/{}/{}", key, value),
            None,
            &[],
        )?;

        let response: Vec<RPCSortitionInfo> = serde_json::from_slice(&bytes)
            .map_err(|_| Error::Deserialize("Failed to decode /v2/info response".into()))?;

        Ok(response)
    }
}

/// This module serde encodes and decodes optional byte fields in RPC
/// responses as Some(String) where the String is a `0x` prefixed
/// hex string.
pub mod prefix_opt_hex {
    pub fn serialize<S: serde::Serializer, T: std::fmt::LowerHex>(
        val: &Option<T>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        match val {
            Some(ref some_val) => {
                let val_str = format!("0x{some_val:x}");
                s.serialize_some(&val_str)
            }
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>, T: super::HexDeser>(
        d: D,
    ) -> Result<Option<T>, D::Error> {
        let opt_inst_str: Option<String> = serde::Deserialize::deserialize(d)?;
        let Some(inst_str) = opt_inst_str else {
            return Ok(None);
        };
        let Some(hex_str) = inst_str.get(2..) else {
            return Err(serde::de::Error::invalid_length(
                inst_str.len(),
                &"at least length 2 string",
            ));
        };
        let val = T::try_from(&hex_str).map_err(serde::de::Error::custom)?;
        Ok(Some(val))
    }
}

/// This module serde encodes and decodes byte fields in RPC
/// responses as a String where the String is a `0x` prefixed
/// hex string.
pub mod prefix_hex {
    pub fn serialize<S: serde::Serializer, T: std::fmt::LowerHex>(
        val: &T,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("0x{val:x}"))
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>, T: super::HexDeser>(
        d: D,
    ) -> Result<T, D::Error> {
        let inst_str: String = serde::Deserialize::deserialize(d)?;
        let Some(hex_str) = inst_str.get(2..) else {
            return Err(serde::de::Error::invalid_length(
                inst_str.len(),
                &"at least length 2 string",
            ));
        };
        T::try_from(&hex_str).map_err(serde::de::Error::custom)
    }
}

pub trait HexDeser: Sized {
    fn try_from(hex: &str) -> Result<Self, HexError>;
}

macro_rules! impl_hex_deser {
    ($thing:ident) => {
        impl HexDeser for $thing {
            fn try_from(hex: &str) -> Result<Self, HexError> {
                $thing::from_hex(hex)
            }
        }
    };
}

impl_hex_deser!(BurnchainHeaderHash);
impl_hex_deser!(StacksBlockId);
impl_hex_deser!(SortitionId);
impl_hex_deser!(ConsensusHash);
impl_hex_deser!(BlockHeaderHash);
impl_hex_deser!(Hash160);
