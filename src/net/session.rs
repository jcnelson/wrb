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

// Yoinked from stacks-core

use std::collections::HashSet;
use std::io::prelude::*;
use std::io::Read;
use std::net::TcpStream;
use std::{io, mem};

use clarity::vm::types::{QualifiedContractIdentifier, StandardPrincipalData};
use clarity::vm::ContractName;
use sha2::{Digest, Sha512_256};
use stacks_common::codec::{
    read_next, read_next_at_most, read_next_exact, write_next, Error as codec_error,
    StacksMessageCodec, MAX_MESSAGE_LEN, MAX_RELAYERS_LEN, PREAMBLE_ENCODED_SIZE,
};

use stacks_common::types::chainstate::SortitionId;
use stacks_common::types::chainstate::StacksPrivateKey;
use stacks_common::types::chainstate::{BlockHeaderHash, BurnchainHeaderHash};
use stacks_common::types::net::PeerAddress;
use stacks_common::types::StacksPublicKeyBuffer;
use stacks_common::util::hash::{to_hex, DoubleSha256, Hash160, MerkleHashFunc};
use stacks_common::util::log;
use stacks_common::util::retry::BoundReader;
use stacks_common::util::secp256k1::{
    MessageSignature, Secp256k1PrivateKey, Secp256k1PublicKey, MESSAGE_SIGNATURE_ENCODED_SIZE,
};

use crate::net::*;
use crate::runner::RPCPeerInfoData;
use crate::runner::Runner;
use crate::stacks_common::types::{PrivateKey, PublicKey};
use crate::tx::string::UrlString;

use rand::thread_rng;
use rand::Rng;
use rand::RngCore;

use serde::{Deserialize, Serialize};

/// Mocked local peer
#[derive(PartialEq, Clone)]
struct LocalPeer {
    pub network_id: u32,
    pub parent_network_id: u32,
    nonce: [u8; 32],
    pub private_key: Secp256k1PrivateKey,
    pub private_key_expire: u64,

    pub addrbytes: PeerAddress,
    pub port: u16,
    pub services: u16,
    pub data_url: UrlString,
}

impl fmt::Display for LocalPeer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "local.{:x}://(bind={:?})",
            self.network_id,
            &self.addrbytes.to_socketaddr(self.port),
        )
    }
}

impl fmt::Debug for LocalPeer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "local.{:x}://(bind={:?})",
            self.network_id,
            &self.addrbytes.to_socketaddr(self.port),
        )
    }
}

impl LocalPeer {
    pub fn new(
        network_id: u32,
        parent_network_id: u32,
        addrbytes: PeerAddress,
        port: u16,
        privkey: Option<Secp256k1PrivateKey>,
        key_expire: u64,
        data_url: UrlString,
    ) -> LocalPeer {
        let mut pkey = privkey.unwrap_or(Secp256k1PrivateKey::new());
        pkey.set_compress_public(true);

        let mut rng = thread_rng();
        let mut my_nonce = [0u8; 32];

        rng.fill_bytes(&mut my_nonce);

        let addr = addrbytes;
        let port = port;
        let services = (ServiceFlags::RELAY as u16) | (ServiceFlags::RPC as u16);

        wrb_debug!(
            "Will be authenticating p2p messages with the following: public_key = {}, services = {}", &Secp256k1PublicKey::from_private(&pkey).to_hex(), &to_hex(&services.to_be_bytes())
        );

        LocalPeer {
            network_id,
            parent_network_id,
            nonce: my_nonce,
            private_key: pkey,
            private_key_expire: key_expire,
            addrbytes: addr,
            port,
            services,
            data_url,
        }
    }
}

impl HandshakeData {
    fn from_local_peer(local_peer: &LocalPeer) -> HandshakeData {
        let (addrbytes, port) = (local_peer.addrbytes.clone(), local_peer.port);

        // transmit the empty string if our data URL compels us to bind to the anynet address
        let data_url = if local_peer.data_url.has_routable_host() {
            local_peer.data_url.clone()
        } else if let Some(data_port) = local_peer.data_url.get_port() {
            // deduce from public IP
            UrlString::try_from(format!("http://{}", addrbytes.to_socketaddr(data_port)).as_str())
                .unwrap()
        } else {
            // unroutable, so don't bother
            UrlString::try_from("").unwrap()
        };

        HandshakeData {
            addrbytes,
            port,
            services: local_peer.services,
            node_public_key: StacksPublicKeyBuffer::from_public_key(
                &Secp256k1PublicKey::from_private(&local_peer.private_key),
            ),
            expire_block_height: local_peer.private_key_expire,
            data_url,
        }
    }
}

pub struct NodeSession {
    local_peer: LocalPeer,
    peer_info: RPCPeerInfoData,
    burn_block_hash: BurnchainHeaderHash,
    stable_burn_block_hash: BurnchainHeaderHash,
    tcp_socket: TcpStream,
    pub handshake_accept_data: Option<HandshakeData>,
    pub stackerdb_accept_data: Option<StackerDBHandshakeData>,
    seq: u32,
}

impl NodeSession {
    /// Make a StacksMessage.  Sign it and set a sequence number.
    fn make_peer_message(&mut self, payload: StacksMessageType) -> Result<StacksMessage, String> {
        let mut msg = StacksMessage::new(
            self.peer_info.peer_version,
            self.peer_info.network_id,
            self.peer_info.burn_block_height,
            &self.burn_block_hash,
            self.peer_info.stable_burn_block_height,
            &self.stable_burn_block_hash,
            payload,
        );

        msg.sign(self.seq, &self.local_peer.private_key)
            .map_err(|e| format!("Failed to sign message {:?}: {:?}", &msg, &e))?;
        self.seq = self.seq.wrapping_add(1);

        Ok(msg)
    }

    /// Send a p2p message.
    /// Returns error text on failure.
    fn send_peer_message(&mut self, msg: StacksMessage) -> Result<(), String> {
        msg.consensus_serialize(&mut self.tcp_socket)
            .map_err(|e| format!("Failed to send message {:?}: {:?}", &msg, &e))
    }

    /// Receive a p2p message.
    /// Returns error text on failure.
    fn recv_peer_message(&mut self) -> Result<StacksMessage, String> {
        let msg: StacksMessage = read_next(&mut self.tcp_socket)
            .map_err(|e| format!("Failed to receive message: {:?}", &e))?;
        Ok(msg)
    }

    /// Begin a p2p session.
    /// Synthesizes a LocalPeer from the remote peer's responses to /v2/info and /v2/pox.
    /// Performs the initial handshake for you.
    ///
    /// Returns the session handle on success.
    /// Returns error text on failure.
    pub fn begin(data_addr: SocketAddr, replica_peer_addr: SocketAddr) -> Result<Self, String> {
        // get /v2/info
        let peer_info = Runner::run_get_info(&data_addr)
            .map_err(|e| format!("Failed to query /v2/info: {:?}", &e))?;

        // convert `pox_consensus` and `stable_pox_consensus` into their respective burn block
        // hashes
        let sort_info = Runner::run_get_sortition_info(
            &data_addr,
            "consensus",
            &format!("{}", &peer_info.pox_consensus),
        )
        .map_err(|e| format!("Failed to decode response from /v3/sortitions: {:?}", &e))?
        .pop()
        .ok_or_else(|| format!("No sortition returned for {}", &peer_info.pox_consensus))?;

        let stable_sort_info = Runner::run_get_sortition_info(
            &data_addr,
            "consensus",
            &format!("{}", &peer_info.stable_pox_consensus),
        )
        .map_err(|e| format!("Failed to decode response from /v3/sortitions: {:?}", &e))?
        .pop()
        .ok_or_else(|| {
            format!(
                "No sortition returned for {}",
                &peer_info.stable_pox_consensus
            )
        })?;

        let burn_block_hash = sort_info.burn_block_hash;
        let stable_burn_block_hash = stable_sort_info.burn_block_hash;

        let local_peer = LocalPeer::new(
            peer_info.network_id,
            peer_info.parent_network_id,
            PeerAddress::from_socketaddr(&replica_peer_addr),
            replica_peer_addr.port(),
            Some(StacksPrivateKey::new()),
            u64::MAX,
            UrlString::try_from(format!("http://127.0.0.1:{}", data_addr.port()).as_str()).unwrap(),
        );

        let tcp_socket = TcpStream::connect(&replica_peer_addr)
            .map_err(|e| format!("Failed to open {:?}: {:?}", &replica_peer_addr, &e))?;

        let mut session = Self {
            local_peer,
            peer_info,
            burn_block_hash,
            stable_burn_block_hash,
            tcp_socket,
            handshake_accept_data: None,
            stackerdb_accept_data: None,
            seq: 0,
        };

        // perform the handshake
        let handshake_data =
            StacksMessageType::Handshake(HandshakeData::from_local_peer(&session.local_peer));
        let handshake = session.make_peer_message(handshake_data)?;
        session.send_peer_message(handshake)?;

        let resp = session.recv_peer_message()?;

        wrb_debug!("Received {:?}", &resp);

        match resp.payload {
            StacksMessageType::HandshakeAccept(handshake_data) => {
                session.handshake_accept_data = Some(handshake_data.handshake);
            }
            StacksMessageType::StackerDBHandshakeAccept(handshake_data, stackerdb_data) => {
                session.handshake_accept_data = Some(handshake_data.handshake);
                session.stackerdb_accept_data = Some(stackerdb_data);
            }
            x => {
                return Err(format!(
                    "Peer returned unexpected message (expected HandshakeAccept variant): {:?}",
                    &x
                ));
            }
        }

        Ok(session)
    }
}
