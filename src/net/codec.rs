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

use std::collections::HashSet;
use std::io::prelude::*;
use std::io::Read;
use std::{io, mem};

use clarity::vm::types::{QualifiedContractIdentifier, StandardPrincipalData};
use clarity::vm::ContractName;
use sha2::{Digest, Sha512_256};
use stacks_common::codec::{
    read_next, read_next_at_most, read_next_exact, write_next, Error as codec_error,
    StacksMessageCodec, MAX_MESSAGE_LEN, MAX_RELAYERS_LEN, PREAMBLE_ENCODED_SIZE,
};
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
use crate::stacks_common::types::{PrivateKey, PublicKey};
use crate::tx::string::UrlString;

impl Preamble {
    /// Make an empty preamble with the given version and fork-set identifier, and payload length.
    pub fn new(
        peer_version: u32,
        network_id: u32,
        block_height: u64,
        burn_block_hash: &BurnchainHeaderHash,
        stable_block_height: u64,
        stable_burn_block_hash: &BurnchainHeaderHash,
        payload_len: u32,
    ) -> Preamble {
        Preamble {
            peer_version,
            network_id,
            seq: 0,
            burn_block_height: block_height,
            burn_block_hash: burn_block_hash.clone(),
            burn_stable_block_height: stable_block_height,
            burn_stable_block_hash: stable_burn_block_hash.clone(),
            additional_data: 0,
            signature: MessageSignature::empty(),
            payload_len,
        }
    }

    /// Given the serialized message type and bits, sign the resulting message and store the
    /// signature.  message_bits includes the relayers, payload type, and payload.
    pub fn sign(
        &mut self,
        message_bits: &[u8],
        privkey: &Secp256k1PrivateKey,
    ) -> Result<(), Error> {
        let mut digest_bits = [0u8; 32];
        let mut sha2 = Sha512_256::new();

        // serialize the premable with a blank signature
        let old_signature = self.signature.clone();
        self.signature = MessageSignature::empty();

        let mut preamble_bits = vec![];
        self.consensus_serialize(&mut preamble_bits)?;
        self.signature = old_signature;

        sha2.update(&preamble_bits[..]);
        sha2.update(message_bits);

        digest_bits.copy_from_slice(sha2.finalize().as_slice());

        let sig = privkey
            .sign(&digest_bits)
            .map_err(|se| Error::SigningError(se.to_string()))?;

        self.signature = sig;
        Ok(())
    }

    /// Given the serialized message type and bits, verify the signature.
    /// message_bits includes the relayers, payload type, and payload
    pub fn verify(
        &mut self,
        message_bits: &[u8],
        pubkey: &Secp256k1PublicKey,
    ) -> Result<(), Error> {
        let mut digest_bits = [0u8; 32];
        let mut sha2 = Sha512_256::new();

        // serialize the preamble with a blank signature
        let sig_bits = self.signature.clone();
        self.signature = MessageSignature::empty();

        let mut preamble_bits = vec![];
        self.consensus_serialize(&mut preamble_bits)?;
        self.signature = sig_bits;

        sha2.update(&preamble_bits[..]);
        sha2.update(message_bits);

        digest_bits.copy_from_slice(sha2.finalize().as_slice());

        let res = pubkey
            .verify(&digest_bits, &self.signature)
            .map_err(|_ve| Error::VerifyingError("Failed to verify signature".to_string()))?;

        if res {
            Ok(())
        } else {
            Err(Error::VerifyingError(
                "Invalid message signature".to_string(),
            ))
        }
    }
}

impl StacksMessageCodec for Preamble {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), codec_error> {
        write_next(fd, &self.peer_version)?;
        write_next(fd, &self.network_id)?;
        write_next(fd, &self.seq)?;
        write_next(fd, &self.burn_block_height)?;
        write_next(fd, &self.burn_block_hash)?;
        write_next(fd, &self.burn_stable_block_height)?;
        write_next(fd, &self.burn_stable_block_hash)?;
        write_next(fd, &self.additional_data)?;
        write_next(fd, &self.signature)?;
        write_next(fd, &self.payload_len)?;
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<Preamble, codec_error> {
        let peer_version: u32 = read_next(fd)?;
        let network_id: u32 = read_next(fd)?;
        let seq: u32 = read_next(fd)?;
        let burn_block_height: u64 = read_next(fd)?;
        let burn_block_hash: BurnchainHeaderHash = read_next(fd)?;
        let burn_stable_block_height: u64 = read_next(fd)?;
        let burn_stable_block_hash: BurnchainHeaderHash = read_next(fd)?;
        let additional_data: u32 = read_next(fd)?;
        let signature: MessageSignature = read_next(fd)?;
        let payload_len: u32 = read_next(fd)?;

        // minimum is 5 bytes -- a zero-length vector (4 bytes of 0) plus a type identifier (1 byte)
        if payload_len < 5 {
            wrb_test_debug!("Payload len is too small: {}", payload_len);
            return Err(codec_error::DeserializeError(format!(
                "Payload len is too small: {}",
                payload_len
            )));
        }

        if payload_len >= MAX_MESSAGE_LEN {
            wrb_test_debug!("Payload len is too big: {}", payload_len);
            return Err(codec_error::DeserializeError(format!(
                "Payload len is too big: {}",
                payload_len
            )));
        }

        if burn_block_height <= burn_stable_block_height {
            wrb_test_debug!(
                "burn block height {} <= burn stable block height {}",
                burn_block_height,
                burn_stable_block_height
            );
            return Err(codec_error::DeserializeError(format!(
                "Burn block height {} <= burn stable block height {}",
                burn_block_height, burn_stable_block_height
            )));
        }

        Ok(Preamble {
            peer_version,
            network_id,
            seq,
            burn_block_height,
            burn_block_hash,
            burn_stable_block_height,
            burn_stable_block_hash,
            additional_data,
            signature,
            payload_len,
        })
    }
}

impl StacksMessageCodec for NeighborAddress {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), codec_error> {
        write_next(fd, &self.addrbytes)?;
        write_next(fd, &self.port)?;
        write_next(fd, &self.public_key_hash)?;
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<NeighborAddress, codec_error> {
        let addrbytes: PeerAddress = read_next(fd)?;
        let port: u16 = read_next(fd)?;
        let public_key_hash: Hash160 = read_next(fd)?;

        Ok(NeighborAddress {
            addrbytes,
            port,
            public_key_hash,
        })
    }
}

impl StacksMessageCodec for HandshakeData {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), codec_error> {
        write_next(fd, &self.addrbytes)?;
        write_next(fd, &self.port)?;
        write_next(fd, &self.services)?;
        write_next(fd, &self.node_public_key)?;
        write_next(fd, &self.expire_block_height)?;
        write_next(fd, &self.data_url)?;
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<HandshakeData, codec_error> {
        let addrbytes: PeerAddress = read_next(fd)?;
        let port: u16 = read_next(fd)?;
        if port == 0 {
            return Err(codec_error::DeserializeError(
                "Invalid handshake data: port is 0".to_string(),
            ));
        }

        let services: u16 = read_next(fd)?;
        let node_public_key: StacksPublicKeyBuffer = read_next(fd)?;
        let expire_block_height: u64 = read_next(fd)?;
        let data_url: UrlString = read_next(fd)?;
        Ok(HandshakeData {
            addrbytes,
            port,
            services,
            node_public_key,
            expire_block_height,
            data_url,
        })
    }
}

impl StacksMessageCodec for HandshakeAcceptData {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), codec_error> {
        write_next(fd, &self.handshake)?;
        write_next(fd, &self.heartbeat_interval)?;
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<HandshakeAcceptData, codec_error> {
        let handshake: HandshakeData = read_next(fd)?;
        let heartbeat_interval: u32 = read_next(fd)?;
        Ok(HandshakeAcceptData {
            handshake,
            heartbeat_interval,
        })
    }
}

impl NackData {
    pub fn new(error_code: u32) -> NackData {
        NackData { error_code }
    }
}

impl StacksMessageCodec for NackData {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), codec_error> {
        write_next(fd, &self.error_code)?;
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<NackData, codec_error> {
        let error_code: u32 = read_next(fd)?;
        Ok(NackData { error_code })
    }
}

fn contract_id_consensus_serialize<W: Write>(
    fd: &mut W,
    cid: &QualifiedContractIdentifier,
) -> Result<(), codec_error> {
    let addr = &cid.issuer;
    let name = &cid.name;
    write_next(fd, &addr.version())?;
    write_next(fd, &addr.1)?;
    write_next(fd, name)?;
    Ok(())
}

fn contract_id_consensus_deserialize<R: Read>(
    fd: &mut R,
) -> Result<QualifiedContractIdentifier, codec_error> {
    let version: u8 = read_next(fd)?;
    let bytes: [u8; 20] = read_next(fd)?;
    let name: ContractName = read_next(fd)?;
    let qn = QualifiedContractIdentifier::new(
        StacksAddress::new(version, Hash160(bytes))
            .map_err(|_| {
                codec_error::DeserializeError(
                    "Failed to make StacksAddress with given version".into(),
                )
            })?
            .into(),
        name,
    );
    Ok(qn)
}

impl StacksMessageCodec for StackerDBHandshakeData {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), codec_error> {
        if self.smart_contracts.len() > 256 {
            return Err(codec_error::ArrayTooLong);
        }
        // force no more than 256 names in the protocol
        let len_u8: u8 = self.smart_contracts.len().try_into().expect("Unreachable");
        write_next(fd, &self.rc_consensus_hash)?;
        write_next(fd, &len_u8)?;
        for cid in self.smart_contracts.iter() {
            contract_id_consensus_serialize(fd, cid)?;
        }
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<StackerDBHandshakeData, codec_error> {
        let rc_consensus_hash = read_next(fd)?;
        let len_u8: u8 = read_next(fd)?;
        let mut smart_contracts = Vec::with_capacity(len_u8 as usize);
        for _ in 0..len_u8 {
            let cid: QualifiedContractIdentifier = contract_id_consensus_deserialize(fd)?;
            smart_contracts.push(cid);
        }
        Ok(StackerDBHandshakeData {
            rc_consensus_hash,
            smart_contracts,
        })
    }
}

impl StacksMessageCodec for RelayData {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), codec_error> {
        write_next(fd, &self.peer)?;
        write_next(fd, &self.seq)?;
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<RelayData, codec_error> {
        let peer: NeighborAddress = read_next(fd)?;
        let seq: u32 = read_next(fd)?;
        Ok(RelayData { peer, seq })
    }
}

impl StacksMessageCodec for StacksMessageID {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), codec_error> {
        write_next(fd, &(*self as u8))
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<StacksMessageID, codec_error> {
        let as_u8: u8 = read_next(fd)?;
        let id = match as_u8 {
            x if x == StacksMessageID::Handshake as u8 => StacksMessageID::Handshake,
            x if x == StacksMessageID::HandshakeAccept as u8 => StacksMessageID::HandshakeAccept,
            x if x == StacksMessageID::HandshakeReject as u8 => StacksMessageID::HandshakeReject,
            x if x == StacksMessageID::StackerDBHandshakeAccept as u8 => {
                StacksMessageID::StackerDBHandshakeAccept
            }
            _ => {
                return Err(codec_error::DeserializeError(
                    "Unknown message ID".to_string(),
                ));
            }
        };
        Ok(id)
    }
}

impl StacksMessageType {
    pub fn get_message_id(&self) -> StacksMessageID {
        match *self {
            StacksMessageType::Handshake(ref _m) => StacksMessageID::Handshake,
            StacksMessageType::HandshakeAccept(ref _m) => StacksMessageID::HandshakeAccept,
            StacksMessageType::HandshakeReject => StacksMessageID::HandshakeReject,
            StacksMessageType::Nack(ref _m) => StacksMessageID::Nack,
            StacksMessageType::StackerDBHandshakeAccept(ref _h, ref _m) => {
                StacksMessageID::StackerDBHandshakeAccept
            }
        }
    }
}

impl StacksMessageCodec for StacksMessageType {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), codec_error> {
        write_next(fd, &(self.get_message_id() as u8))?;
        match *self {
            StacksMessageType::Handshake(ref m) => write_next(fd, m)?,
            StacksMessageType::HandshakeAccept(ref m) => write_next(fd, m)?,
            StacksMessageType::HandshakeReject => {}
            StacksMessageType::Nack(ref m) => write_next(fd, m)?,
            StacksMessageType::StackerDBHandshakeAccept(ref h, ref m) => {
                write_next(fd, h)?;
                write_next(fd, m)?
            }
        }
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<StacksMessageType, codec_error> {
        let message_id: StacksMessageID = read_next(fd)?;
        let message = match message_id {
            StacksMessageID::Handshake => {
                let m: HandshakeData = read_next(fd)?;
                StacksMessageType::Handshake(m)
            }
            StacksMessageID::HandshakeAccept => {
                let m: HandshakeAcceptData = read_next(fd)?;
                StacksMessageType::HandshakeAccept(m)
            }
            StacksMessageID::HandshakeReject => StacksMessageType::HandshakeReject,
            StacksMessageID::Nack => {
                let m: NackData = read_next(fd)?;
                StacksMessageType::Nack(m)
            }
            StacksMessageID::StackerDBHandshakeAccept => {
                let h: HandshakeAcceptData = read_next(fd)?;
                let m: StackerDBHandshakeData = read_next(fd)?;
                StacksMessageType::StackerDBHandshakeAccept(h, m)
            }
        };
        Ok(message)
    }
}

impl StacksMessageCodec for StacksMessage {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), codec_error> {
        write_next(fd, &self.preamble)?;
        write_next(fd, &self.relayers)?;
        write_next(fd, &self.payload)?;
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<StacksMessage, codec_error> {
        let preamble: Preamble = read_next(fd)?;
        if preamble.payload_len > MAX_MESSAGE_LEN - PREAMBLE_ENCODED_SIZE {
            return Err(codec_error::DeserializeError(
                "Message would be too big".to_string(),
            ));
        }

        let relayers: Vec<RelayData> = read_next_at_most::<_, RelayData>(fd, MAX_RELAYERS_LEN)?;
        let payload: StacksMessageType = read_next(fd)?;

        let message = StacksMessage {
            preamble,
            relayers,
            payload,
        };
        Ok(message)
    }
}

impl StacksMessage {
    /// Create an unsigned Stacks p2p message
    pub fn new(
        peer_version: u32,
        network_id: u32,
        block_height: u64,
        burn_header_hash: &BurnchainHeaderHash,
        stable_block_height: u64,
        stable_burn_header_hash: &BurnchainHeaderHash,
        message: StacksMessageType,
    ) -> StacksMessage {
        let preamble = Preamble::new(
            peer_version,
            network_id,
            block_height,
            burn_header_hash,
            stable_block_height,
            stable_burn_header_hash,
            0,
        );
        StacksMessage {
            preamble,
            relayers: vec![],
            payload: message,
        }
    }

    /// Sign the stacks message
    fn do_sign(&mut self, private_key: &Secp256k1PrivateKey) -> Result<(), Error> {
        let mut message_bits = vec![];
        self.relayers.consensus_serialize(&mut message_bits)?;
        self.payload.consensus_serialize(&mut message_bits)?;

        self.preamble.payload_len = message_bits.len() as u32;
        self.preamble.sign(&message_bits[..], private_key)
    }

    /// Sign the StacksMessage.  The StacksMessage must _not_ have any relayers (i.e. we're
    /// originating this messsage).
    pub fn sign(&mut self, seq: u32, private_key: &Secp256k1PrivateKey) -> Result<(), Error> {
        if !self.relayers.is_empty() {
            return Err(Error::InvalidMessage);
        }
        self.preamble.seq = seq;
        self.do_sign(private_key)
    }

    /// Sign the StacksMessage and add ourselves as a relayer.
    pub fn sign_relay(
        &mut self,
        private_key: &Secp256k1PrivateKey,
        our_seq: u32,
        our_addr: &NeighborAddress,
    ) -> Result<(), Error> {
        if self.relayers.len() >= MAX_RELAYERS_LEN as usize {
            wrb_warn!("Message has too many relayers; will not sign",);
            return Err(Error::InvalidMessage);
        }

        // don't sign if signed more than once
        for relayer in &self.relayers {
            if relayer.peer.public_key_hash == our_addr.public_key_hash {
                wrb_warn!("Message already signed by {}", &our_addr.public_key_hash);
                return Err(Error::InvalidMessage);
            }
        }

        // save relayer state
        let our_relay = RelayData {
            peer: our_addr.clone(),
            seq: self.preamble.seq,
        };

        self.relayers.push(our_relay);
        self.preamble.seq = our_seq;
        self.do_sign(private_key)
    }

    pub fn deserialize_body<R: Read>(
        fd: &mut R,
    ) -> Result<(Vec<RelayData>, StacksMessageType), Error> {
        let relayers: Vec<RelayData> = read_next_at_most::<_, RelayData>(fd, MAX_RELAYERS_LEN)?;
        let payload: StacksMessageType = read_next(fd)?;
        Ok((relayers, payload))
    }

    /// Verify this message by treating the public key buffer as a secp256k1 public key.
    /// Fails if:
    /// * the signature doesn't match
    /// * the buffer doesn't encode a secp256k1 public key
    pub fn verify_secp256k1(&self, public_key: &StacksPublicKeyBuffer) -> Result<(), Error> {
        let secp256k1_pubkey = public_key
            .to_public_key()
            .map_err(|e| Error::DeserializeError(e.into()))?;

        let mut message_bits = vec![];
        self.relayers.consensus_serialize(&mut message_bits)?;
        self.payload.consensus_serialize(&mut message_bits)?;

        let mut p = self.preamble.clone();
        p.verify(&message_bits, &secp256k1_pubkey).map(|_m| ())
    }
}
