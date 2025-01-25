// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020 Stacks Open Internet Foundation
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

/// Lifted with gratitude from stacks-core
use std::io::prelude::*;
use std::io::{Read, Write};
use std::ops::{Deref, DerefMut};
use std::{error, fmt, io};

use clarity::vm::representations::{ClarityName, ContractName};
use clarity::vm::types::{
    PrincipalData, QualifiedContractIdentifier, StandardPrincipalData, Value,
};
use clarity::vm::ClarityVersion;
use serde::{Deserialize, Serialize};
use stacks_common::address::AddressHashMode;
use stacks_common::codec::{
    read_next, write_next, Error as codec_error, StacksMessageCodec, MAX_MESSAGE_LEN,
};
use stacks_common::deps_common::bitcoin::util::hash::Sha256dHash;
use stacks_common::types::chainstate::StacksPublicKey;
use stacks_common::types::chainstate::{
    BlockHeaderHash, BurnchainHeaderHash, StacksAddress, StacksBlockId, StacksWorkScore, TrieHash,
    TRIEHASH_ENCODED_SIZE,
};
use stacks_common::util::hash::{
    hex_bytes, to_hex, Hash160, Sha512Trunc256Sum, HASH160_ENCODED_SIZE,
};

use stacks_common::consts::{CHAIN_ID_MAINNET, CHAIN_ID_TESTNET};
use stacks_common::types::chainstate::ConsensusHash;
use stacks_common::util::secp256k1;
use stacks_common::util::secp256k1::MessageSignature;
use stacks_common::util::vrf::VRFProof;

use crate::tx::string::StacksString;

use stacks_common::types::chainstate::StacksPrivateKey;

pub mod auth;
pub mod string;
pub mod transaction;

#[derive(Debug)]
pub enum Error {
    SigningError(String),
    VerifyingError(String),
    Incomplete,
    Codec(codec_error),
    IncompatibleSpendingConditionError,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::SigningError(ref s) => write!(f, "Failed to sign: {}", s),
            Error::VerifyingError(ref s) => write!(f, "Failed to verify signature: {}", s),
            Error::Codec(ref e) => write!(f, "Codec error: {:?}", e),
            Error::Incomplete => write!(f, "Incompletely signed transaction"),
            Error::IncompatibleSpendingConditionError => {
                write!(f, "Incompatible spending condition error")
            }
        }
    }
}

impl error::Error for Error {
    fn cause(&self) -> Option<&dyn error::Error> {
        match self {
            Error::SigningError(..) => None,
            Error::VerifyingError(..) => None,
            Error::Codec(ref e) => Some(e),
            Error::Incomplete => None,
            Error::IncompatibleSpendingConditionError => None,
        }
    }
}

impl From<codec_error> for Error {
    fn from(e: codec_error) -> Self {
        Self::Codec(e)
    }
}

pub struct Txid(pub [u8; 32]);
impl_array_newtype!(Txid, u8, 32);
impl_array_hexstring_fmt!(Txid);
impl_byte_array_newtype!(Txid, u8, 32);
impl_byte_array_message_codec!(Txid, 32);
impl_byte_array_serde!(Txid);
pub const TXID_ENCODED_SIZE: u32 = 32;

impl Txid {
    /// A Stacks transaction ID is a sha512/256 hash (not a double-sha256 hash)
    pub fn from_stacks_tx(txdata: &[u8]) -> Txid {
        let h = Sha512Trunc256Sum::from_data(txdata);
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(h.as_bytes());
        Txid(bytes)
    }

    /// A sighash is calculated the same way as a txid
    pub fn from_sighash_bytes(txdata: &[u8]) -> Txid {
        Txid::from_stacks_tx(txdata)
    }

    /// Create a Txid from the tx hash bytes used in bitcoin.
    /// This just reverses the inner bytes of the input.
    pub fn from_bitcoin_tx_hash(tx_hash: &Sha256dHash) -> Txid {
        let mut txid_bytes = tx_hash.0.clone();
        txid_bytes.reverse();
        Self(txid_bytes)
    }
}

pub const MAX_TRANSACTION_LEN: u32 = 2 * 1024 * 1024;

/// How a transaction may be appended to the Stacks blockchain
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Copy, Serialize, Deserialize)]
pub enum TransactionAnchorMode {
    OnChainOnly = 1,  // must be included in a StacksBlock
    OffChainOnly = 2, // must be included in a StacksMicroBlock
    Any = 3,          // either
}

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Copy, Serialize, Deserialize)]
pub enum TransactionAuthFlags {
    // types of auth
    AuthStandard = 0x04,
    AuthSponsored = 0x05,
}

/// Transaction signatures are validated by calculating the public key from the signature, and
/// verifying that all public keys hash to the signing account's hash.  To do so, we must preserve
/// enough information in the auth structure to recover each public key's bytes.
///
/// An auth field can be a public key or a signature.  In both cases, the public key (either given
/// in-the-raw or embedded in a signature) may be encoded as compressed or uncompressed.
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Copy, Serialize, Deserialize)]
pub enum TransactionAuthFieldID {
    // types of auth fields
    PublicKeyCompressed = 0x00,
    PublicKeyUncompressed = 0x01,
    SignatureCompressed = 0x02,
    SignatureUncompressed = 0x03,
}

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Copy, Serialize, Deserialize)]
pub enum TransactionPublicKeyEncoding {
    // ways we can encode a public key
    Compressed = 0x00,
    Uncompressed = 0x01,
}

impl TransactionPublicKeyEncoding {
    pub fn from_u8(n: u8) -> Option<TransactionPublicKeyEncoding> {
        match n {
            x if x == TransactionPublicKeyEncoding::Compressed as u8 => {
                Some(TransactionPublicKeyEncoding::Compressed)
            }
            x if x == TransactionPublicKeyEncoding::Uncompressed as u8 => {
                Some(TransactionPublicKeyEncoding::Uncompressed)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransactionAuthField {
    PublicKey(StacksPublicKey),
    Signature(TransactionPublicKeyEncoding, MessageSignature),
}

impl TransactionAuthField {
    pub fn is_public_key(&self) -> bool {
        match *self {
            TransactionAuthField::PublicKey(_) => true,
            _ => false,
        }
    }

    pub fn is_signature(&self) -> bool {
        match *self {
            TransactionAuthField::Signature(_, _) => true,
            _ => false,
        }
    }

    pub fn as_public_key(&self) -> Option<StacksPublicKey> {
        match *self {
            TransactionAuthField::PublicKey(ref pubk) => Some(pubk.clone()),
            _ => None,
        }
    }

    pub fn as_signature(&self) -> Option<(TransactionPublicKeyEncoding, MessageSignature)> {
        match *self {
            TransactionAuthField::Signature(ref key_fmt, ref sig) => {
                Some((key_fmt.clone(), sig.clone()))
            }
            _ => None,
        }
    }

    pub fn get_public_key(&self, sighash_bytes: &[u8]) -> Result<StacksPublicKey, Error> {
        match *self {
            TransactionAuthField::PublicKey(ref pubk) => Ok(pubk.clone()),
            TransactionAuthField::Signature(ref key_fmt, ref sig) => {
                let mut pubk = StacksPublicKey::recover_to_pubkey(sighash_bytes, sig)
                    .map_err(|e| Error::VerifyingError(e.to_string()))?;
                pubk.set_compressed(if *key_fmt == TransactionPublicKeyEncoding::Compressed {
                    true
                } else {
                    false
                });
                Ok(pubk)
            }
        }
    }
}

// tag address hash modes as "singlesig" or "multisig" so we can't accidentally construct an
// invalid spending condition
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SinglesigHashMode {
    P2PKH = 0x00,
    P2WPKH = 0x02,
}

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MultisigHashMode {
    P2SH = 0x01,
    P2WSH = 0x03,
}

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OrderIndependentMultisigHashMode {
    P2SH = 0x05,
    P2WSH = 0x07,
}

impl SinglesigHashMode {
    pub fn to_address_hash_mode(&self) -> AddressHashMode {
        match *self {
            SinglesigHashMode::P2PKH => AddressHashMode::SerializeP2PKH,
            SinglesigHashMode::P2WPKH => AddressHashMode::SerializeP2WPKH,
        }
    }

    pub fn from_address_hash_mode(hm: AddressHashMode) -> Option<SinglesigHashMode> {
        match hm {
            AddressHashMode::SerializeP2PKH => Some(SinglesigHashMode::P2PKH),
            AddressHashMode::SerializeP2WPKH => Some(SinglesigHashMode::P2WPKH),
            _ => None,
        }
    }

    pub fn from_u8(n: u8) -> Option<SinglesigHashMode> {
        match n {
            x if x == SinglesigHashMode::P2PKH as u8 => Some(SinglesigHashMode::P2PKH),
            x if x == SinglesigHashMode::P2WPKH as u8 => Some(SinglesigHashMode::P2WPKH),
            _ => None,
        }
    }
}

impl MultisigHashMode {
    pub fn to_address_hash_mode(&self) -> AddressHashMode {
        match *self {
            MultisigHashMode::P2SH => AddressHashMode::SerializeP2SH,
            MultisigHashMode::P2WSH => AddressHashMode::SerializeP2WSH,
        }
    }

    pub fn from_address_hash_mode(hm: AddressHashMode) -> Option<MultisigHashMode> {
        match hm {
            AddressHashMode::SerializeP2SH => Some(MultisigHashMode::P2SH),
            AddressHashMode::SerializeP2WSH => Some(MultisigHashMode::P2WSH),
            _ => None,
        }
    }

    pub fn from_u8(n: u8) -> Option<MultisigHashMode> {
        match n {
            x if x == MultisigHashMode::P2SH as u8 => Some(MultisigHashMode::P2SH),
            x if x == MultisigHashMode::P2WSH as u8 => Some(MultisigHashMode::P2WSH),
            _ => None,
        }
    }
}

impl OrderIndependentMultisigHashMode {
    pub fn to_address_hash_mode(&self) -> AddressHashMode {
        match *self {
            OrderIndependentMultisigHashMode::P2SH => AddressHashMode::SerializeP2SH,
            OrderIndependentMultisigHashMode::P2WSH => AddressHashMode::SerializeP2WSH,
        }
    }

    pub fn from_address_hash_mode(hm: AddressHashMode) -> Option<OrderIndependentMultisigHashMode> {
        match hm {
            AddressHashMode::SerializeP2SH => Some(OrderIndependentMultisigHashMode::P2SH),
            AddressHashMode::SerializeP2WSH => Some(OrderIndependentMultisigHashMode::P2WSH),
            _ => None,
        }
    }

    pub fn from_u8(n: u8) -> Option<OrderIndependentMultisigHashMode> {
        match n {
            x if x == OrderIndependentMultisigHashMode::P2SH as u8 => {
                Some(OrderIndependentMultisigHashMode::P2SH)
            }
            x if x == OrderIndependentMultisigHashMode::P2WSH as u8 => {
                Some(OrderIndependentMultisigHashMode::P2WSH)
            }
            _ => None,
        }
    }
}

/// A structure that encodes enough state to authenticate
/// a transaction's execution against a Stacks address.
/// public_keys + signatures_required determines the Principal.
/// nonce is the "check number" for the Principal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultisigSpendingCondition {
    pub hash_mode: MultisigHashMode,
    pub signer: Hash160,
    pub nonce: u64,  // nth authorization from this account
    pub tx_fee: u64, // microSTX/compute rate offered by this account
    pub fields: Vec<TransactionAuthField>,
    pub signatures_required: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SinglesigSpendingCondition {
    pub hash_mode: SinglesigHashMode,
    pub signer: Hash160,
    pub nonce: u64,  // nth authorization from this account
    pub tx_fee: u64, // microSTX/compute rate offerred by this account
    pub key_encoding: TransactionPublicKeyEncoding,
    pub signature: MessageSignature,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderIndependentMultisigSpendingCondition {
    pub hash_mode: OrderIndependentMultisigHashMode,
    pub signer: Hash160,
    pub nonce: u64,  // nth authorization from this account
    pub tx_fee: u64, // microSTX/compute rate offered by this account
    pub fields: Vec<TransactionAuthField>,
    pub signatures_required: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransactionSpendingCondition {
    Singlesig(SinglesigSpendingCondition),
    Multisig(MultisigSpendingCondition),
    OrderIndependentMultisig(OrderIndependentMultisigSpendingCondition),
}

/// Types of transaction authorizations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransactionAuth {
    Standard(TransactionSpendingCondition),
    Sponsored(TransactionSpendingCondition, TransactionSpendingCondition), // the second account pays on behalf of the first account
}

/// A transaction that calls into a smart contract
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransactionContractCall {
    pub address: StacksAddress,
    pub contract_name: ContractName,
    pub function_name: ClarityName,
    pub function_args: Vec<Value>,
}

impl TransactionContractCall {
    pub fn contract_identifier(&self) -> QualifiedContractIdentifier {
        let standard_principal = StandardPrincipalData::from(self.address.clone());
        QualifiedContractIdentifier::new(standard_principal, self.contract_name.clone())
    }
}

/// A transaction that instantiates a smart contract
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransactionSmartContract {
    pub name: ContractName,
    pub code_body: StacksString,
}

pub struct TokenTransferMemo(pub [u8; 34]); // same length as it is in stacks v1
impl_byte_array_message_codec!(TokenTransferMemo, 34);
impl_array_newtype!(TokenTransferMemo, u8, 34);
impl_array_hexstring_fmt!(TokenTransferMemo);
impl_byte_array_newtype!(TokenTransferMemo, u8, 34);
impl_byte_array_serde!(TokenTransferMemo);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransactionPayload {
    TokenTransfer(PrincipalData, u64, TokenTransferMemo),
    ContractCall(TransactionContractCall),
    SmartContract(TransactionSmartContract, Option<ClarityVersion>),
}

impl TransactionPayload {
    pub fn name(&self) -> &'static str {
        match self {
            TransactionPayload::TokenTransfer(..) => "TokenTransfer",
            TransactionPayload::ContractCall(..) => "ContractCall",
            TransactionPayload::SmartContract(_, version_opt) => {
                if version_opt.is_some() {
                    "SmartContract(Versioned)"
                } else {
                    "SmartContract"
                }
            }
        }
    }
}

define_u8_enum!(TransactionPayloadID {
    TokenTransfer = 0,
    SmartContract = 1,
    ContractCall = 2,
    VersionedSmartContract = 6
});

/// Encoding of an asset type identifier
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetInfo {
    pub contract_address: StacksAddress,
    pub contract_name: ContractName,
    pub asset_name: ClarityName,
}

/// numeric wire-format ID of an asset info type variant
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Copy, Serialize, Deserialize)]
pub enum AssetInfoID {
    STX = 0,
    FungibleAsset = 1,
    NonfungibleAsset = 2,
}

impl AssetInfoID {
    pub fn from_u8(b: u8) -> Option<AssetInfoID> {
        match b {
            0 => Some(AssetInfoID::STX),
            1 => Some(AssetInfoID::FungibleAsset),
            2 => Some(AssetInfoID::NonfungibleAsset),
            _ => None,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Copy, Serialize, Deserialize)]
pub enum FungibleConditionCode {
    SentEq = 0x01,
    SentGt = 0x02,
    SentGe = 0x03,
    SentLt = 0x04,
    SentLe = 0x05,
}

impl FungibleConditionCode {
    pub fn from_u8(b: u8) -> Option<FungibleConditionCode> {
        match b {
            0x01 => Some(FungibleConditionCode::SentEq),
            0x02 => Some(FungibleConditionCode::SentGt),
            0x03 => Some(FungibleConditionCode::SentGe),
            0x04 => Some(FungibleConditionCode::SentLt),
            0x05 => Some(FungibleConditionCode::SentLe),
            _ => None,
        }
    }

    pub fn check(&self, amount_sent_condition: u128, amount_sent: u128) -> bool {
        match *self {
            FungibleConditionCode::SentEq => amount_sent == amount_sent_condition,
            FungibleConditionCode::SentGt => amount_sent > amount_sent_condition,
            FungibleConditionCode::SentGe => amount_sent >= amount_sent_condition,
            FungibleConditionCode::SentLt => amount_sent < amount_sent_condition,
            FungibleConditionCode::SentLe => amount_sent <= amount_sent_condition,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Copy, Serialize, Deserialize)]
pub enum NonfungibleConditionCode {
    Sent = 0x10,
    NotSent = 0x11,
}

impl NonfungibleConditionCode {
    pub fn from_u8(b: u8) -> Option<NonfungibleConditionCode> {
        match b {
            0x10 => Some(NonfungibleConditionCode::Sent),
            0x11 => Some(NonfungibleConditionCode::NotSent),
            _ => None,
        }
    }

    pub fn was_sent(nft_sent_condition: &Value, nfts_sent: &[Value]) -> bool {
        for asset_sent in nfts_sent.iter() {
            if *asset_sent == *nft_sent_condition {
                // asset was sent, and is no longer owned by this principal
                return true;
            }
        }
        return false;
    }

    pub fn check(&self, nft_sent_condition: &Value, nfts_sent: &[Value]) -> bool {
        match *self {
            NonfungibleConditionCode::Sent => {
                NonfungibleConditionCode::was_sent(nft_sent_condition, nfts_sent)
            }
            NonfungibleConditionCode::NotSent => {
                !NonfungibleConditionCode::was_sent(nft_sent_condition, nfts_sent)
            }
        }
    }
}

/// Post-condition principal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PostConditionPrincipal {
    Origin,
    Standard(StacksAddress),
    Contract(StacksAddress, ContractName),
}

impl PostConditionPrincipal {
    pub fn to_principal_data(&self, origin_principal: &PrincipalData) -> PrincipalData {
        match *self {
            PostConditionPrincipal::Origin => origin_principal.clone(),
            PostConditionPrincipal::Standard(ref addr) => {
                PrincipalData::Standard(StandardPrincipalData::from(addr.clone()))
            }
            PostConditionPrincipal::Contract(ref addr, ref contract_name) => {
                PrincipalData::Contract(QualifiedContractIdentifier::new(
                    StandardPrincipalData::from(addr.clone()),
                    contract_name.clone(),
                ))
            }
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Copy, Serialize, Deserialize)]
pub enum PostConditionPrincipalID {
    Origin = 0x01,
    Standard = 0x02,
    Contract = 0x03,
}

/// Post-condition on a transaction
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransactionPostCondition {
    STX(PostConditionPrincipal, FungibleConditionCode, u64),
    Fungible(
        PostConditionPrincipal,
        AssetInfo,
        FungibleConditionCode,
        u64,
    ),
    Nonfungible(
        PostConditionPrincipal,
        AssetInfo,
        Value,
        NonfungibleConditionCode,
    ),
}

/// Post-condition modes for unspecified assets
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Copy, Serialize, Deserialize)]
pub enum TransactionPostConditionMode {
    Allow = 0x01, // allow any other changes not specified
    Deny = 0x02,  // deny any other changes not specified
}

/// Stacks transaction versions
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Copy, Serialize, Deserialize)]
pub enum TransactionVersion {
    Mainnet = 0x00,
    Testnet = 0x80,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StacksTransaction {
    pub version: TransactionVersion,
    pub chain_id: u32,
    pub auth: TransactionAuth,
    pub anchor_mode: TransactionAnchorMode,
    pub post_condition_mode: TransactionPostConditionMode,
    pub post_conditions: Vec<TransactionPostCondition>,
    pub payload: TransactionPayload,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StacksTransactionSigner {
    pub tx: StacksTransaction,
    pub sighash: Txid,
    origin_done: bool,
    check_oversign: bool,
    check_overlap: bool,
}

pub fn serialize_sign_tx(
    version: TransactionVersion,
    payload: TransactionPayload,
    sender: &StacksPrivateKey,
    payer: Option<&StacksPrivateKey>,
    sender_nonce: u64,
    payer_nonce: Option<u64>,
    tx_fee: u64,
    post_condition_mode: TransactionPostConditionMode,
    post_conditions: Vec<TransactionPostCondition>,
) -> Result<StacksTransaction, Error> {
    let chain_id = if version == TransactionVersion::Mainnet {
        CHAIN_ID_MAINNET
    } else {
        CHAIN_ID_TESTNET
    };

    let mut sender_spending_condition =
        TransactionSpendingCondition::new_singlesig_p2pkh(StacksPublicKey::from_private(sender))
            .expect("Failed to create p2pkh spending condition from public key.");
    sender_spending_condition.set_nonce(sender_nonce);

    let auth = match (payer, payer_nonce) {
        (Some(payer), Some(payer_nonce)) => {
            let mut payer_spending_condition = TransactionSpendingCondition::new_singlesig_p2pkh(
                StacksPublicKey::from_private(payer),
            )
            .expect("Failed to create p2pkh spending condition from public key.");
            payer_spending_condition.set_nonce(payer_nonce);
            payer_spending_condition.set_tx_fee(tx_fee);
            TransactionAuth::Sponsored(sender_spending_condition, payer_spending_condition)
        }
        _ => {
            sender_spending_condition.set_tx_fee(tx_fee);
            TransactionAuth::Standard(sender_spending_condition)
        }
    };
    let mut unsigned_tx = StacksTransaction::new(version, auth, payload);
    unsigned_tx.anchor_mode = TransactionAnchorMode::Any;
    unsigned_tx.post_condition_mode = post_condition_mode;
    unsigned_tx.post_conditions = post_conditions;
    unsigned_tx.chain_id = chain_id;

    let mut tx_signer = StacksTransactionSigner::new(&unsigned_tx);
    tx_signer.sign_origin(sender)?;
    if let (Some(payer), Some(_)) = (payer, payer_nonce) {
        tx_signer.sign_sponsor(payer)?;
    }

    tx_signer.get_tx().ok_or(Error::Incomplete)
}

pub fn make_contract_call(
    mainnet: bool,
    sender: &StacksPrivateKey,
    nonce: u64,
    tx_fee: u64,
    contract_addr: &StacksAddress,
    contract_name: &str,
    function_name: &str,
    function_args: &[Value],
    post_condition_mode: TransactionPostConditionMode,
    post_conditions: Vec<TransactionPostCondition>,
) -> Result<StacksTransaction, Error> {
    let contract_name = ContractName::from(contract_name);
    let function_name = ClarityName::from(function_name);

    let payload = TransactionContractCall {
        address: *contract_addr,
        contract_name,
        function_name,
        function_args: function_args.to_vec(),
    };

    let version = if mainnet {
        TransactionVersion::Mainnet
    } else {
        TransactionVersion::Testnet
    };

    serialize_sign_tx(
        version,
        payload.into(),
        sender,
        None,
        nonce,
        None,
        tx_fee,
        post_condition_mode,
        post_conditions,
    )
}

pub fn make_contract_publish_versioned(
    mainnet: bool,
    sender: &StacksPrivateKey,
    nonce: u64,
    tx_fee: u64,
    contract_name: &str,
    contract_content: &str,
    version: Option<ClarityVersion>,
    post_condition_mode: TransactionPostConditionMode,
    post_conditions: Vec<TransactionPostCondition>,
) -> Result<StacksTransaction, Error> {
    let name = ContractName::from(contract_name);
    let code_body =
        StacksString::from_string(&contract_content.to_string()).ok_or(Error::Codec(
            codec_error::DeserializeError("Failer to decode string to StackString".into()),
        ))?;

    let payload =
        TransactionPayload::SmartContract(TransactionSmartContract { name, code_body }, version);

    let version = if mainnet {
        TransactionVersion::Mainnet
    } else {
        TransactionVersion::Testnet
    };

    serialize_sign_tx(
        version,
        payload,
        sender,
        None,
        nonce,
        None,
        tx_fee,
        post_condition_mode,
        post_conditions,
    )
}

pub fn make_contract_publish(
    mainnet: bool,
    sender: &StacksPrivateKey,
    nonce: u64,
    tx_fee: u64,
    contract_name: &str,
    contract_content: &str,
    post_condition_mode: TransactionPostConditionMode,
    post_conditions: Vec<TransactionPostCondition>,
) -> Result<StacksTransaction, Error> {
    make_contract_publish_versioned(
        mainnet,
        sender,
        nonce,
        tx_fee,
        contract_name,
        contract_content,
        None,
        post_condition_mode,
        post_conditions,
    )
}
