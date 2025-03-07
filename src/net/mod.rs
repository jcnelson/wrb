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

/// Yoinked from stacks-core
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::io::prelude::*;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::ops::Deref;
use std::str::FromStr;
use std::{error, fmt, io};

use clarity::vm::types::{PrincipalData, QualifiedContractIdentifier, StandardPrincipalData};
use clarity::vm::{ClarityName, ContractName, Value};
use libstackerdb::{
    Error as libstackerdb_error, SlotMetadata, StackerDBChunkAckData, StackerDBChunkData,
};
use stacks_common::codec::{
    read_next, write_next, Error as codec_error, StacksMessageCodec,
    BURNCHAIN_HEADER_HASH_ENCODED_SIZE,
};
use stacks_common::types::chainstate::ConsensusHash;
use stacks_common::types::chainstate::{
    BlockHeaderHash, BurnchainHeaderHash, PoxId, StacksAddress, StacksBlockId,
};
use stacks_common::types::net::{Error as AddrError, PeerAddress, PeerHost};
use stacks_common::types::StacksPublicKeyBuffer;
use stacks_common::util::hash::{
    hex_bytes, to_hex, Hash160, Sha256Sum, DOUBLE_SHA256_ENCODED_SIZE, HASH160_ENCODED_SIZE,
};
use stacks_common::util::secp256k1::{
    MessageSignature, Secp256k1PublicKey, MESSAGE_SIGNATURE_ENCODED_SIZE,
};
use stacks_common::util::{get_epoch_time_secs, log};

use crate::tx::string::UrlString;

use serde;
use serde::{Deserialize, Serialize};

pub mod codec;
pub mod session;

#[derive(Debug)]
pub enum Error {
    /// Failed to encode
    SerializeError(String),
    /// Failed to read
    ReadError(io::Error),
    /// Failed to decode
    DeserializeError(String),
    /// Failed to write
    WriteError(io::Error),
    /// Underflow -- not enough bytes to form the message
    UnderflowError(String),
    /// Overflow -- message too big
    OverflowError(String),
    /// Wrong protocol family
    WrongProtocolFamily,
    /// Array is too big
    ArrayTooLong,
    /// Receive timed out
    RecvTimeout,
    /// Error signing a message
    SigningError(String),
    /// Error verifying a message
    VerifyingError(String),
    /// Read stream is drained.  Try again
    TemporarilyDrained,
    /// Read stream has reached EOF (socket closed, end-of-file reached, etc.)
    PermanentlyDrained,
    /// Failed to read from the FS
    FilesystemError,
    /// Socket mutex was poisoned
    SocketMutexPoisoned,
    /// Socket not instantiated
    SocketNotConnectedToPeer,
    /// Not connected to peer
    ConnectionBroken,
    /// Connection could not be (re-)established
    ConnectionError,
    /// Too many outgoing messages
    OutboxOverflow,
    /// Too many incoming messages
    InboxOverflow,
    /// Send error
    SendError(String),
    /// Recv error
    RecvError(String),
    /// Invalid message
    InvalidMessage,
    /// Invalid network handle
    InvalidHandle,
    /// Network handle is full
    FullHandle,
    /// Invalid handshake
    InvalidHandshake,
    /// Stale neighbor
    StaleNeighbor,
    /// No such neighbor
    NoSuchNeighbor,
    /// Failed to bind
    BindError,
    /// Failed to poll
    PollError,
    /// Failed to accept
    AcceptError,
    /// Failed to register socket with poller
    RegisterError,
    /// Failed to query socket metadata
    SocketError,
    /// server is not bound to a socket
    NotConnected,
    /// Remote peer is not connected
    PeerNotConnected,
    /// Too many peers
    TooManyPeers,
    /// Message already in progress
    InProgress,
    /// Peer is denied
    Denied,
    /// Data URL is not known
    NoDataUrl,
    /// Peer is transmitting too fast
    PeerThrottled,
    /// Error resolving a DNS name
    LookupError(String),
    /// Coordinator hung up
    CoordinatorClosed,
    /// view of state is stale (e.g. from the sortition db)
    StaleView,
    /// Tried to connect to myself
    ConnectionCycle,
    /// Requested data not found
    NotFoundError,
    /// Transient error (akin to EAGAIN)
    Transient(String),
    /// Expected end-of-stream, but had more data
    ExpectedEndOfStream,
    /// chunk is stale
    StaleChunk {
        supplied_version: u32,
        latest_version: u32,
    },
    /// no such slot
    NoSuchSlot(QualifiedContractIdentifier, u32),
    /// no such DB
    NoSuchStackerDB(QualifiedContractIdentifier),
    /// stacker DB exists
    StackerDBExists(QualifiedContractIdentifier),
    /// slot signer is wrong
    BadSlotSigner(StacksAddress, u32),
    /// too many writes to a slot
    TooManySlotWrites {
        supplied_version: u32,
        max_writes: u32,
    },
    /// too frequent writes to a slot
    TooFrequentSlotWrites(u64),
    /// Invalid control smart contract for a Stacker DB
    InvalidStackerDBContract(QualifiedContractIdentifier, String),
    /// state machine step took too long
    StepTimeout,
    /// stacker DB chunk is too big
    StackerDBChunkTooBig(usize),
    /// Invalid state machine state reached
    InvalidState,
}

impl From<libstackerdb_error> for Error {
    fn from(e: libstackerdb_error) -> Self {
        match e {
            libstackerdb_error::SigningError(s) => Error::SigningError(s),
            libstackerdb_error::VerifyingError(s) => Error::VerifyingError(s),
        }
    }
}

impl From<codec_error> for Error {
    fn from(e: codec_error) -> Self {
        match e {
            codec_error::SerializeError(s) => Error::SerializeError(s),
            codec_error::ReadError(e) => Error::ReadError(e),
            codec_error::DeserializeError(s) => Error::DeserializeError(s),
            codec_error::WriteError(e) => Error::WriteError(e),
            codec_error::UnderflowError(s) => Error::UnderflowError(s),
            codec_error::OverflowError(s) => Error::OverflowError(s),
            codec_error::ArrayTooLong => Error::ArrayTooLong,
            codec_error::SigningError(s) => Error::SigningError(s),
            codec_error::GenericError(_) => Error::InvalidMessage,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::SerializeError(ref s) => fmt::Display::fmt(s, f),
            Error::DeserializeError(ref s) => fmt::Display::fmt(s, f),
            Error::ReadError(ref io) => fmt::Display::fmt(io, f),
            Error::WriteError(ref io) => fmt::Display::fmt(io, f),
            Error::UnderflowError(ref s) => fmt::Display::fmt(s, f),
            Error::OverflowError(ref s) => fmt::Display::fmt(s, f),
            Error::WrongProtocolFamily => write!(f, "Improper use of protocol family"),
            Error::ArrayTooLong => write!(f, "Array too long"),
            Error::RecvTimeout => write!(f, "Packet receive timeout"),
            Error::SigningError(ref s) => fmt::Display::fmt(s, f),
            Error::VerifyingError(ref s) => fmt::Display::fmt(s, f),
            Error::TemporarilyDrained => {
                write!(f, "Temporarily out of bytes to read; try again later")
            }
            Error::PermanentlyDrained => write!(f, "Out of bytes to read"),
            Error::FilesystemError => write!(f, "Disk I/O error"),
            Error::SocketMutexPoisoned => write!(f, "socket mutex was poisoned"),
            Error::SocketNotConnectedToPeer => write!(f, "not connected to peer"),
            Error::ConnectionBroken => write!(f, "connection to peer node is broken"),
            Error::ConnectionError => write!(f, "connection to peer could not be (re-)established"),
            Error::OutboxOverflow => write!(f, "too many outgoing messages queued"),
            Error::InboxOverflow => write!(f, "too many messages pending"),
            Error::SendError(ref s) => fmt::Display::fmt(s, f),
            Error::RecvError(ref s) => fmt::Display::fmt(s, f),
            Error::InvalidMessage => write!(f, "invalid message (malformed or bad signature)"),
            Error::InvalidHandle => write!(f, "invalid network handle"),
            Error::FullHandle => write!(f, "network handle is full and needs to be drained"),
            Error::InvalidHandshake => write!(f, "invalid handshake from remote peer"),
            Error::StaleNeighbor => write!(f, "neighbor is too far behind the chain tip"),
            Error::NoSuchNeighbor => write!(f, "no such neighbor"),
            Error::BindError => write!(f, "Failed to bind to the given address"),
            Error::PollError => write!(f, "Failed to poll"),
            Error::AcceptError => write!(f, "Failed to accept connection"),
            Error::RegisterError => write!(f, "Failed to register socket with poller"),
            Error::SocketError => write!(f, "Socket error"),
            Error::NotConnected => write!(f, "Not connected to peer network"),
            Error::PeerNotConnected => write!(f, "Remote peer is not connected to us"),
            Error::TooManyPeers => write!(f, "Too many peer connections open"),
            Error::InProgress => write!(f, "Message already in progress"),
            Error::Denied => write!(f, "Peer is denied"),
            Error::NoDataUrl => write!(f, "No data URL available"),
            Error::PeerThrottled => write!(f, "Peer is transmitting too fast"),
            Error::LookupError(ref s) => fmt::Display::fmt(s, f),
            Error::CoordinatorClosed => write!(f, "Coordinator hung up"),
            Error::StaleView => write!(f, "State view is stale"),
            Error::ConnectionCycle => write!(f, "Tried to connect to myself"),
            Error::NotFoundError => write!(f, "Requested data not found"),
            Error::Transient(ref s) => write!(f, "Transient network error: {}", s),
            Error::ExpectedEndOfStream => write!(f, "Expected end-of-stream"),
            Error::StaleChunk {
                supplied_version,
                latest_version,
            } => {
                write!(
                    f,
                    "Stale DB chunk (supplied={},latest={})",
                    supplied_version, latest_version
                )
            }
            Error::NoSuchSlot(ref addr, ref slot_id) => {
                write!(f, "No such DB slot ({},{})", addr, slot_id)
            }
            Error::NoSuchStackerDB(ref addr) => {
                write!(f, "No such StackerDB {}", addr)
            }
            Error::StackerDBExists(ref addr) => {
                write!(f, "StackerDB already exists: {}", addr)
            }
            Error::BadSlotSigner(ref addr, ref slot_id) => {
                write!(f, "Bad DB slot signer ({},{})", addr, slot_id)
            }
            Error::TooManySlotWrites {
                supplied_version,
                max_writes,
            } => {
                write!(
                    f,
                    "Too many slot writes (max={},given={})",
                    max_writes, supplied_version
                )
            }
            Error::TooFrequentSlotWrites(ref deadline) => {
                write!(f, "Too frequent slot writes (deadline={})", deadline)
            }
            Error::InvalidStackerDBContract(ref contract_id, ref reason) => {
                write!(
                    f,
                    "Invalid StackerDB control smart contract {}: {}",
                    contract_id, reason
                )
            }
            Error::StepTimeout => write!(f, "State-machine step took too long"),
            Error::StackerDBChunkTooBig(ref sz) => {
                write!(f, "StackerDB chunk size is too big ({})", sz)
            }
            Error::InvalidState => write!(f, "Invalid state-machine state reached"),
        }
    }
}

impl error::Error for Error {
    fn cause(&self) -> Option<&dyn error::Error> {
        match *self {
            Error::SerializeError(ref _s) => None,
            Error::ReadError(ref io) => Some(io),
            Error::DeserializeError(ref _s) => None,
            Error::WriteError(ref io) => Some(io),
            Error::UnderflowError(ref _s) => None,
            Error::OverflowError(ref _s) => None,
            Error::WrongProtocolFamily => None,
            Error::ArrayTooLong => None,
            Error::RecvTimeout => None,
            Error::SigningError(ref _s) => None,
            Error::VerifyingError(ref _s) => None,
            Error::TemporarilyDrained => None,
            Error::PermanentlyDrained => None,
            Error::FilesystemError => None,
            Error::SocketMutexPoisoned => None,
            Error::SocketNotConnectedToPeer => None,
            Error::ConnectionBroken => None,
            Error::ConnectionError => None,
            Error::OutboxOverflow => None,
            Error::InboxOverflow => None,
            Error::SendError(ref _s) => None,
            Error::RecvError(ref _s) => None,
            Error::InvalidMessage => None,
            Error::InvalidHandle => None,
            Error::FullHandle => None,
            Error::InvalidHandshake => None,
            Error::StaleNeighbor => None,
            Error::NoSuchNeighbor => None,
            Error::BindError => None,
            Error::PollError => None,
            Error::AcceptError => None,
            Error::RegisterError => None,
            Error::SocketError => None,
            Error::NotConnected => None,
            Error::PeerNotConnected => None,
            Error::TooManyPeers => None,
            Error::InProgress => None,
            Error::Denied => None,
            Error::NoDataUrl => None,
            Error::PeerThrottled => None,
            Error::LookupError(ref _s) => None,
            Error::CoordinatorClosed => None,
            Error::StaleView => None,
            Error::ConnectionCycle => None,
            Error::NotFoundError => None,
            Error::Transient(ref _s) => None,
            Error::ExpectedEndOfStream => None,
            Error::StaleChunk { .. } => None,
            Error::NoSuchSlot(..) => None,
            Error::NoSuchStackerDB(..) => None,
            Error::StackerDBExists(..) => None,
            Error::BadSlotSigner(..) => None,
            Error::TooManySlotWrites { .. } => None,
            Error::TooFrequentSlotWrites(..) => None,
            Error::InvalidStackerDBContract(..) => None,
            Error::StepTimeout => None,
            Error::StackerDBChunkTooBig(..) => None,
            Error::InvalidState => None,
        }
    }
}

/// P2P message preamble -- included in all p2p network messages
#[derive(Debug, Clone, PartialEq)]
pub struct Preamble {
    pub peer_version: u32,                           // software version
    pub network_id: u32,                             // mainnet, testnet, etc.
    pub seq: u32, // message sequence number -- pairs this message to a request
    pub burn_block_height: u64, // last-seen block height (at chain tip)
    pub burn_block_hash: BurnchainHeaderHash, // hash of the last-seen burn block
    pub burn_stable_block_height: u64, // latest stable block height (e.g. chain tip minus 7)
    pub burn_stable_block_hash: BurnchainHeaderHash, // latest stable burnchain header hash.
    pub additional_data: u32, // RESERVED; pointer to additional data (should be all 0's if not used)
    pub signature: MessageSignature, // signature from the peer that sent this
    pub payload_len: u32,     // length of the following payload, including relayers vector
}

/// A descriptor of a peer
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NeighborAddress {
    #[serde(rename = "ip")]
    pub addrbytes: PeerAddress,
    pub port: u16,
    pub public_key_hash: Hash160, // used as a hint; useful for when a node trusts another node to be honest about this
}

impl fmt::Display for NeighborAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:?}://{:?}",
            &self.public_key_hash,
            &self.addrbytes.to_socketaddr(self.port)
        )
    }
}

impl fmt::Debug for NeighborAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:?}://{:?}",
            &self.public_key_hash,
            &self.addrbytes.to_socketaddr(self.port)
        )
    }
}

impl NeighborAddress {
    pub fn clear_public_key(&mut self) {
        self.public_key_hash = Hash160([0u8; 20]);
    }

    pub fn to_socketaddr(&self) -> SocketAddr {
        self.addrbytes.to_socketaddr(self.port)
    }
}

/// Handshake request -- this is the first message sent to a peer.
/// The remote peer will reply a HandshakeAccept with just a preamble
/// if the peer accepts.  Otherwise it will get a HandshakeReject with just
/// a preamble.
///
/// To keep peer knowledge fresh, nodes will send handshakes to each other
/// as heartbeat messages.
#[derive(Debug, Clone, PartialEq)]
pub struct HandshakeData {
    pub addrbytes: PeerAddress,
    pub port: u16,
    pub services: u16, // bit field representing services this node offers
    pub node_public_key: StacksPublicKeyBuffer,
    pub expire_block_height: u64, // burn block height after which this node's key will be revoked,
    pub data_url: UrlString,
}

#[repr(u8)]
pub enum ServiceFlags {
    RELAY = 0x01,
    RPC = 0x02,
    STACKERDB = 0x04,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HandshakeAcceptData {
    pub handshake: HandshakeData, // this peer's handshake information
    pub heartbeat_interval: u32,  // hint as to how long this peer will remember you
}

/// Inform the remote peer of (a page of) the list of stacker DB contracts this node supports
#[derive(Debug, Clone, PartialEq)]
pub struct StackerDBHandshakeData {
    /// current reward cycle consensus hash (i.e. the consensus hash of the Stacks tip in the
    /// current reward cycle, which commits to both the Stacks block tip and the underlying PoX
    /// history).
    pub rc_consensus_hash: ConsensusHash,
    /// list of smart contracts that we index.
    /// there can be as many as 256 entries.
    pub smart_contracts: Vec<QualifiedContractIdentifier>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NackData {
    pub error_code: u32,
}
pub mod NackErrorCodes {
    /// A handshake has not yet been completed with the requester
    /// and it is required before the protocol can proceed
    pub const HandshakeRequired: u32 = 1;
    /// The request depends on a burnchain block that this peer does not recognize
    pub const NoSuchBurnchainBlock: u32 = 2;
    /// The remote peer has exceeded local per-peer bandwidth limits
    pub const Throttled: u32 = 3;
    /// The request depends on a PoX fork that this peer does not recognize as canonical
    pub const InvalidPoxFork: u32 = 4;
    /// The message received is not appropriate for the ongoing step in the protocol being executed
    pub const InvalidMessage: u32 = 5;
    /// The StackerDB requested is not known or configured on this node
    pub const NoSuchDB: u32 = 6;
    /// The StackerDB chunk request referred to an older copy of the chunk than this node has
    pub const StaleVersion: u32 = 7;
    /// The remote peer's view of the burnchain is too out-of-date for the protocol to continue
    pub const StaleView: u32 = 8;
    /// The StackerDB chunk request referred to a newer copy of the chunk that this node has
    pub const FutureVersion: u32 = 9;
    /// The referenced StackerDB state view is stale locally relative to the requested version
    pub const FutureView: u32 = 10;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RelayData {
    pub peer: NeighborAddress,
    pub seq: u32,
}

/// All P2P message types supported in the wrb client
#[derive(Debug, Clone, PartialEq)]
pub enum StacksMessageType {
    Handshake(HandshakeData),
    HandshakeAccept(HandshakeAcceptData),
    HandshakeReject,
    Nack(NackData),
    StackerDBHandshakeAccept(HandshakeAcceptData, StackerDBHandshakeData),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StacksMessageID {
    Handshake = 0,
    HandshakeAccept = 1,
    HandshakeReject = 2,
    Nack = 14,
    StackerDBHandshakeAccept = 19,
}

/// Message type for all P2P Stacks network messages
#[derive(Debug, Clone, PartialEq)]
pub struct StacksMessage {
    pub preamble: Preamble,
    pub relayers: Vec<RelayData>,
    pub payload: StacksMessageType,
}
