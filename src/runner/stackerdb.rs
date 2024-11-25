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

use clarity::vm::types::QualifiedContractIdentifier;

use crate::core::Config;
use crate::runner::http::*;
use crate::runner::Runner;
use crate::runner::Error;
use crate::storage::StackerDBClient;

use stacks_common::types::chainstate::StacksAddress;

use libstackerdb::*;

use serde_json;

pub struct StackerDBSession {
    /// host we're talking to
    pub host: SocketAddr,
    /// contract we're talking to
    pub stackerdb_contract_id: QualifiedContractIdentifier,
    /// connection to the replica
    sock: Option<TcpStream>,
}

impl StackerDBSession {
    /// instantiate but don't connect
    pub fn new(
        host: SocketAddr,
        stackerdb_contract_id: QualifiedContractIdentifier,
    ) -> StackerDBSession {
        StackerDBSession {
            host,
            stackerdb_contract_id,
            sock: None,
        }
    }

    /// connect or reconnect to the node
    fn connect_or_reconnect(&mut self) -> Result<(), Error> {
        wrb_debug!("connect to {}", &self.host);
        self.sock = Some(TcpStream::connect(self.host)?);
        Ok(())
    }

    /// Do something with the connected socket
    fn with_socket<F, R>(&mut self, todo: F) -> Result<R, Error>
    where
        F: FnOnce(&mut StackerDBSession, &mut TcpStream) -> R,
    {
        self.connect_or_reconnect()?;
        let mut sock = if let Some(s) = self.sock.take() {
            s
        } else {
            return Err(Error::NotConnected);
        };

        let res = todo(self, &mut sock);

        self.sock = Some(sock);
        Ok(res)
    }

    /// send an HTTP RPC request and receive a reply.
    /// Return the HTTP reply, decoded if it was chunked
    fn rpc_request(
        &mut self,
        verb: &str,
        path: &str,
        content_type: Option<&str>,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        self.with_socket(|session, sock| {
            run_http_request(sock, &session.host, verb, path, content_type, payload)
        })?
    }

    /// connect to the replica
    pub fn connect(
        &mut self,
        host: SocketAddr,
        stackerdb_contract_id: QualifiedContractIdentifier,
    ) -> Result<(), Error> {
        self.host = host;
        self.stackerdb_contract_id = stackerdb_contract_id;
        self.connect_or_reconnect()
    }
}

impl StackerDBClient for StackerDBSession {
    /// query the replica for a list of chunks
    fn list_chunks(&mut self) -> Result<Vec<SlotMetadata>, Error> {
        let bytes = self.rpc_request(
            "GET",
            &stackerdb_get_metadata_path(self.stackerdb_contract_id.clone()),
            None,
            &[],
        )?;
        let metadata: Vec<SlotMetadata> =
            serde_json::from_slice(&bytes).map_err(|e| Error::Deserialize(format!("{:?}", &e)))?;
        Ok(metadata)
    }

    /// query the replica for zero or more chunks
    fn get_chunks(
        &mut self,
        slots_and_versions: &[(u32, u32)],
    ) -> Result<Vec<Option<Vec<u8>>>, Error> {
        let mut payloads = vec![];
        for (slot_id, slot_version) in slots_and_versions.iter() {
            let path = stackerdb_get_chunk_path(
                self.stackerdb_contract_id.clone(),
                *slot_id,
                Some(*slot_version),
            );
            let chunk = match self.rpc_request("GET", &path, None, &[]) {
                Ok(body_bytes) => Some(body_bytes),
                Err(Error::HttpError(code)) => {
                    if code != 404 {
                        return Err(Error::HttpError(code));
                    }
                    None
                }
                Err(e) => {
                    return Err(e);
                }
            };
            payloads.push(chunk);
        }
        Ok(payloads)
    }

    /// query the replica for zero or more latest chunks
    fn get_latest_chunks(&mut self, slot_ids: &[u32]) -> Result<Vec<Option<Vec<u8>>>, Error> {
        let mut payloads = vec![];
        for slot_id in slot_ids.iter() {
            let path = stackerdb_get_chunk_path(self.stackerdb_contract_id.clone(), *slot_id, None);
            let chunk = match self.rpc_request("GET", &path, None, &[]) {
                Ok(body_bytes) => Some(body_bytes),
                Err(Error::HttpError(code)) => {
                    if code != 404 {
                        return Err(Error::HttpError(code));
                    }
                    None
                }
                Err(e) => {
                    return Err(e);
                }
            };
            payloads.push(chunk);
        }
        Ok(payloads)
    }

    /// upload a chunk
    fn put_chunk(&mut self, chunk: StackerDBChunkData) -> Result<StackerDBChunkAckData, Error> {
        let body =
            serde_json::to_vec(&chunk).map_err(|e| Error::Deserialize(format!("{:?}", &e)))?;
        let path = stackerdb_post_chunk_path(self.stackerdb_contract_id.clone());
        let resp_bytes = self.rpc_request("POST", &path, Some("application/json"), &body)?;
        let ack: StackerDBChunkAckData = serde_json::from_slice(&resp_bytes)
            .map_err(|e| Error::Deserialize(format!("{:?}", &e)))?;
        Ok(ack)
    }

    /// Find the list of replicas
    fn find_replicas(&mut self) -> Result<Vec<SocketAddr>, Error> {
        Runner::run_get_stackerdb_replicas(&self.host, &self.stackerdb_contract_id)
    }

    /// Get the signers for a StackerDB.
    /// Return the list of addresses for each slot
    fn get_signers(&mut self) -> Result<Vec<StacksAddress>, Error> {
        Runner::run_get_stackerdb_signers(&self.host, &self.stackerdb_contract_id)
    }
}
