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

use std::convert::TryFrom;
use std::error;
use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::net::ToSocketAddrs;

use crate::core::Config;
use crate::core::with_global_config;

use crate::runner::http::run_http_request;

use clarity::vm::types::QualifiedContractIdentifier;
use clarity::vm::types::StandardPrincipalData;
use clarity::vm::Value;

use stacks_common::types::chainstate::StacksAddress;
use stacks_common::util::hash::{hex_bytes, Hash160};

use serde::Deserialize;
use serde::Serialize;

use clarity::vm::errors::Error as clarity_error;
use clarity::vm::errors::InterpreterError as clarity_interpreter_error;

pub mod bns;
pub mod http;
pub mod process;
pub mod stackerdb;

#[cfg(test)]
pub mod tests;

#[derive(Debug, Clone)]
pub enum Error {
    FailedToRun(String),
    FailedToExecute(String, String),
    KilledBySignal(String),
    BadExit(i32),
    InvalidOutput(String),
    IO(String),
    Deserialize(String),
    NotConnected,
    NotInitialized,
    MalformedRequest(String),
    MalformedResponse(String),
    HttpError(u32),
    RPCError(String),
    Clarity(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::FailedToRun(ref cmd) => write!(f, "Failed to run '{}'", cmd),
            Error::FailedToExecute(ref cmd, ref ioe) => {
                write!(f, "Failed to run '{}': {}", cmd, ioe)
            }
            Error::KilledBySignal(ref cmd) => {
                write!(f, "Failed to run '{}': killed by signal", cmd)
            }
            Error::BadExit(ref es) => write!(f, "Command exited with status {}", es),
            Error::InvalidOutput(ref s) => write!(f, "Invalid command output: '{}'", s),
            Error::IO(ref e) => write!(f, "IO Error: {}", e),
            Error::Deserialize(ref s) => write!(f, "Deserialize error: {}", s),
            Error::NotConnected => write!(f, "Not connected"),
            Error::NotInitialized => write!(f, "System not initialized"),
            Error::MalformedRequest(ref s) => write!(f, "Malformed request: {}", s),
            Error::MalformedResponse(ref s) => write!(f, "Malformed response: {}", s),
            Error::HttpError(ref code) => write!(f, "Bad HTTP code: {}", code),
            Error::RPCError(ref msg) => write!(f, "RPC error: {}", msg),
            Error::Clarity(ref err) => write!(f, "Clarity error: {}", err),
        }
    }
}

impl error::Error for Error {
    fn cause(&self) -> Option<&dyn error::Error> {
        match *self {
            Error::FailedToRun(_) => None,
            Error::FailedToExecute(..) => None,
            Error::KilledBySignal(_) => None,
            Error::BadExit(_) => None,
            Error::InvalidOutput(_) => None,
            Error::IO(..) => None,
            Error::Deserialize(_) => None,
            Error::NotConnected => None,
            Error::NotInitialized => None,
            Error::MalformedRequest(_) => None,
            Error::MalformedResponse(_) => None,
            Error::HttpError(_) => None,
            Error::RPCError(_) => None,
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
        let (node_host, node_port) = with_global_config(|cfg| cfg.get_node_addr())
            .ok_or(Error::NotInitialized)?;
        if self.node.is_none() {
            let mut addrs: Vec<_> = (node_host.as_str(), node_port).to_socket_addrs()?.collect();
            return Ok(addrs.pop());
        }
        Ok(self.node.clone())
    }

    /// Run a read-only function call on the node
    pub fn call_readonly(
        &mut self,
        contract_id: &QualifiedContractIdentifier,
        function_name: &str,
        function_args: &[Value],
    ) -> Result<Value, Error> {
        let Some(node_addr) = self.resolve_node()? else {
            return Err(Error::NotConnected);
        };
        let mut sock = TcpStream::connect(&node_addr)?;

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
        let bytes = run_http_request(
            &mut sock,
            &node_addr,
            "POST",
            &format!(
                "/v2/contracts/call-read/{}/{}/{}",
                &contract_id.issuer, &contract_id.name, function_name
            ),
            Some("application/json"),
            payload_json.as_bytes(),
        )?;

        debug!("call-readonly: {}", &payload_json);

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

    /// Get an attachment from Atlas
    pub fn get_attachment(&mut self, attachment_hash: &Hash160) -> Result<Vec<u8>, Error> {
        let Some(node_addr) = self.resolve_node()? else {
            return Err(Error::NotConnected);
        };
        let mut sock = TcpStream::connect(&node_addr)?;
        let bytes = run_http_request(
            &mut sock,
            &node_addr,
            "GET",
            &format!("/v2/attachments/{}", attachment_hash),
            None,
            &[],
        )?;

        let response_hex: String = serde_json::from_slice(&bytes)
            .map_err(|_| Error::Deserialize("Failed to decode attachment response bytes".into()))?;
        let response = hex_bytes(&response_hex).map_err(|_| {
            Error::Deserialize("Failed to decode attachment: not a hex string".into())
        })?;
        Ok(response)
    }
}
