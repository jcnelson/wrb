// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2025 Stacks Open Internet Foundation
// Copyright (C) 2025 Jude Nelson
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
use std::net::SocketAddr;
use std::net::TcpStream;

use crate::core::Config;
use crate::runner::run_http_request;
use crate::runner::Error;
use crate::runner::Runner;

use crate::tx::StacksTransaction;
use crate::tx::Txid;

use clarity::vm::types::BufferLength;
use clarity::vm::types::PrincipalData;
use clarity::vm::types::QualifiedContractIdentifier;
use clarity::vm::types::ResponseData;
use clarity::vm::types::SequenceData;
use clarity::vm::types::Value;

use clarity::vm::costs::ExecutionCost;

use stacks_common::codec::StacksMessageCodec;
use stacks_common::util::hash::{to_hex, Hash160};

use serde::{Deserialize, Serialize};
use serde_json;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct AccountEntryResponse {
    pub balance: String,
    pub locked: String,
    pub unlock_height: u64,
    pub nonce: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub balance_proof: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub nonce_proof: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StacksAccount {
    pub balance: u128,
    pub locked: u128,
    pub nonce: u64,
}

#[derive(Serialize, Deserialize)]
pub struct FeeRateEstimateRequestBody {
    #[serde(default)]
    pub estimated_len: Option<u64>,
    pub transaction_payload: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RPCFeeEstimate {
    pub fee_rate: f64,
    pub fee: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RPCFeeEstimateResponse {
    pub estimated_cost: ExecutionCost,
    pub estimated_cost_scalar: u64,
    pub estimations: Vec<RPCFeeEstimate>,
    pub cost_scalar_change_by_byte: f64,
}

impl TryFrom<AccountEntryResponse> for StacksAccount {
    type Error = Error;
    fn try_from(a: AccountEntryResponse) -> Result<Self, Error> {
        let balance = u128::from_str_radix(&a.balance[2..], 16)
            .map_err(|_| Error::Deserialize("Failed to decode balance".into()))?;

        let locked = u128::from_str_radix(&a.locked[2..], 16)
            .map_err(|_| Error::Deserialize("Failed to decode locked".into()))?;

        Ok(Self {
            balance,
            locked,
            nonce: a.nonce,
        })
    }
}

impl Runner {
    pub fn run_get_account(
        node_addr: &SocketAddr,
        account: &PrincipalData,
    ) -> Result<StacksAccount, Error> {
        let mut sock = TcpStream::connect(node_addr)?;
        let bytes = run_http_request(
            &mut sock,
            node_addr,
            "GET",
            &format!("/v2/accounts/{}?proof=0", &account.to_string()),
            None,
            &[],
        )?;

        let response: AccountEntryResponse = serde_json::from_slice(&bytes)
            .map_err(|_| Error::Deserialize("Failed to decode account".into()))?;

        StacksAccount::try_from(response)
    }

    pub fn get_account(&mut self, account: &PrincipalData) -> Result<StacksAccount, Error> {
        let Some(node_addr) = self.resolve_node()? else {
            return Err(Error::NotConnected);
        };
        Self::run_get_account(&node_addr, account)
    }

    pub fn run_get_tx_fee(
        node_addr: &SocketAddr,
        tx: &StacksTransaction,
    ) -> Result<RPCFeeEstimateResponse, Error> {
        let tx_bytes = tx.serialize_to_vec();
        let tx_payload_bytes = tx.payload.serialize_to_vec();
        let tx_payload_hex = to_hex(&tx_payload_bytes);

        let request_body = FeeRateEstimateRequestBody {
            estimated_len: u64::try_from(tx_bytes.len()).ok(),
            transaction_payload: tx_payload_hex,
        };

        let request_body_json = serde_json::to_string(&request_body)
            .map_err(|_| Error::Serialize("Failed to encode request to JSON".into()))?;

        let mut sock = TcpStream::connect(node_addr)?;
        let bytes = match run_http_request(
            &mut sock,
            node_addr,
            "POST",
            "/v2/fees/transaction",
            Some("application/json"),
            request_body_json.as_bytes(),
        ) {
            Ok(bytes) => bytes,
            Err(Error::HttpError(code, headers, offset)) => {
                if code == 400 {
                    // no estimate available
                    return Err(Error::NoFeeEstimate);
                } else {
                    // other HTTP error
                    return Err(Error::HttpError(code, headers, offset));
                }
            }
            Err(e) => {
                return Err(e);
            }
        };

        let response: RPCFeeEstimateResponse = serde_json::from_slice(&bytes)
            .map_err(|_| Error::Deserialize("Failed to decode fee rate".into()))?;

        Ok(response)
    }

    pub fn get_tx_fee(&mut self, tx: &StacksTransaction) -> Result<RPCFeeEstimateResponse, Error> {
        let Some(node_addr) = self.resolve_node()? else {
            return Err(Error::NotConnected);
        };
        Self::run_get_tx_fee(&node_addr, tx)
    }

    pub fn run_post_tx(node_addr: &SocketAddr, tx: &StacksTransaction) -> Result<Txid, Error> {
        let tx_bytes = tx.serialize_to_vec();

        let mut sock = TcpStream::connect(node_addr)?;
        let bytes = run_http_request(
            &mut sock,
            node_addr,
            "POST",
            "/v2/transactions",
            Some("application/octet-stream"),
            &tx_bytes,
        )?;

        let response: Txid = serde_json::from_slice(&bytes)
            .map_err(|_| Error::Deserialize("Failed to decode txid".into()))?;

        Ok(response)
    }

    pub fn post_tx(&mut self, tx: &StacksTransaction) -> Result<Txid, Error> {
        let Some(node_addr) = self.resolve_node()? else {
            return Err(Error::NotConnected);
        };
        Self::run_post_tx(&node_addr, tx)
    }
}
