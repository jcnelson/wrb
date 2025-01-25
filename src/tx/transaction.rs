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

use std::io;
use std::io::prelude::*;
use std::io::{Read, Write};

use clarity::vm::representations::{ClarityName, ContractName};
use clarity::vm::types::serialization::SerializationError as clarity_serialization_error;
use clarity::vm::types::{
    QualifiedContractIdentifier, SequenceData, SequencedValue, StandardPrincipalData,
    MAX_TYPE_DEPTH,
};
use clarity::vm::{ClarityVersion, SymbolicExpression, SymbolicExpressionType, Value};
use stacks_common::codec::{read_next, write_next, Error as codec_error, StacksMessageCodec};
use stacks_common::types::chainstate::StacksAddress;
use stacks_common::types::chainstate::StacksPrivateKey;
use stacks_common::types::StacksPublicKeyBuffer;
use stacks_common::util::hash::{to_hex, MerkleHashFunc, MerkleTree, Sha512Trunc256Sum};
use stacks_common::util::retry::BoundReader;
use stacks_common::util::secp256k1::MessageSignature;
use stacks_common::util::secp256k1::Secp256k1PrivateKey;

use crate::tx::*;

impl StacksMessageCodec for TransactionContractCall {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), codec_error> {
        write_next(fd, &self.address)?;
        write_next(fd, &self.contract_name)?;
        write_next(fd, &self.function_name)?;
        write_next(fd, &self.function_args)?;
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<TransactionContractCall, codec_error> {
        let address: StacksAddress = read_next(fd)?;
        let contract_name: ContractName = read_next(fd)?;
        let function_name: ClarityName = read_next(fd)?;
        let function_args: Vec<Value> = {
            let mut bound_read = BoundReader::from_reader(fd, u64::from(MAX_TRANSACTION_LEN));
            read_next(&mut bound_read)
        }?;

        // function name must be valid Clarity variable
        if !StacksString::from(function_name.clone()).is_clarity_variable() {
            return Err(codec_error::DeserializeError(
                "Failed to parse transaction: invalid function name -- not a Clarity variable"
                    .to_string(),
            ));
        }

        Ok(TransactionContractCall {
            address,
            contract_name,
            function_name,
            function_args,
        })
    }
}

impl TransactionContractCall {
    pub fn to_clarity_contract_id(&self) -> QualifiedContractIdentifier {
        QualifiedContractIdentifier::new(
            StandardPrincipalData::from(self.address.clone()),
            self.contract_name.clone(),
        )
    }
}

impl fmt::Display for TransactionContractCall {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let formatted_args = self
            .function_args
            .iter()
            .map(|v| format!("{}", v))
            .collect::<Vec<String>>()
            .join(", ");
        write!(
            f,
            "{}.{}::{}({})",
            self.address, self.contract_name, self.function_name, formatted_args
        )
    }
}

impl StacksMessageCodec for TransactionSmartContract {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), codec_error> {
        write_next(fd, &self.name)?;
        write_next(fd, &self.code_body)?;
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<TransactionSmartContract, codec_error> {
        let name: ContractName = read_next(fd)?;
        let code_body: StacksString = read_next(fd)?;
        Ok(TransactionSmartContract { name, code_body })
    }
}

fn ClarityVersion_consensus_serialize<W: Write>(
    version: &ClarityVersion,
    fd: &mut W,
) -> Result<(), codec_error> {
    match *version {
        ClarityVersion::Clarity1 => write_next(fd, &1u8)?,
        ClarityVersion::Clarity2 => write_next(fd, &2u8)?,
        ClarityVersion::Clarity3 => write_next(fd, &3u8)?,
    }
    Ok(())
}

fn ClarityVersion_consensus_deserialize<R: Read>(
    fd: &mut R,
) -> Result<ClarityVersion, codec_error> {
    let version_byte: u8 = read_next(fd)?;
    match version_byte {
        1u8 => Ok(ClarityVersion::Clarity1),
        2u8 => Ok(ClarityVersion::Clarity2),
        3u8 => Ok(ClarityVersion::Clarity3),
        _ => Err(codec_error::DeserializeError(format!(
            "Unrecognized ClarityVersion byte {}",
            &version_byte
        ))),
    }
}

impl StacksMessageCodec for TransactionPayload {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), codec_error> {
        match self {
            TransactionPayload::TokenTransfer(address, amount, memo) => {
                write_next(fd, &(TransactionPayloadID::TokenTransfer as u8))?;
                write_next(fd, address)?;
                write_next(fd, amount)?;
                write_next(fd, memo)?;
            }
            TransactionPayload::ContractCall(cc) => {
                write_next(fd, &(TransactionPayloadID::ContractCall as u8))?;
                cc.consensus_serialize(fd)?;
            }
            TransactionPayload::SmartContract(sc, version_opt) => {
                if let Some(version) = version_opt {
                    // caller requests a specific Clarity version
                    write_next(fd, &(TransactionPayloadID::VersionedSmartContract as u8))?;
                    ClarityVersion_consensus_serialize(&version, fd)?;
                    sc.consensus_serialize(fd)?;
                } else {
                    // caller requests to use whatever the current clarity version is
                    write_next(fd, &(TransactionPayloadID::SmartContract as u8))?;
                    sc.consensus_serialize(fd)?;
                }
            }
        }
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<TransactionPayload, codec_error> {
        let type_id_u8 = read_next(fd)?;
        let type_id = TransactionPayloadID::from_u8(type_id_u8).ok_or_else(|| {
            codec_error::DeserializeError(format!(
                "Failed to parse transaction -- unknown payload ID {type_id_u8}"
            ))
        })?;
        let payload = match type_id {
            TransactionPayloadID::TokenTransfer => {
                let principal = read_next(fd)?;
                let amount = read_next(fd)?;
                let memo = read_next(fd)?;
                TransactionPayload::TokenTransfer(principal, amount, memo)
            }
            TransactionPayloadID::ContractCall => {
                let payload: TransactionContractCall = read_next(fd)?;
                TransactionPayload::ContractCall(payload)
            }
            TransactionPayloadID::SmartContract => {
                let payload: TransactionSmartContract = read_next(fd)?;
                TransactionPayload::SmartContract(payload, None)
            }
            TransactionPayloadID::VersionedSmartContract => {
                let version = ClarityVersion_consensus_deserialize(fd)?;
                let payload: TransactionSmartContract = read_next(fd)?;
                TransactionPayload::SmartContract(payload, Some(version))
            }
        };

        Ok(payload)
    }
}

impl<'a, H> FromIterator<&'a StacksTransaction> for MerkleTree<H>
where
    H: MerkleHashFunc + Clone + PartialEq + fmt::Debug,
{
    fn from_iter<T: IntoIterator<Item = &'a StacksTransaction>>(iter: T) -> Self {
        let txid_vec: Vec<_> = iter
            .into_iter()
            .map(|x| x.txid().as_bytes().to_vec())
            .collect();
        MerkleTree::new(&txid_vec)
    }
}

impl TransactionPayload {
    pub fn new_contract_call(
        contract_address: StacksAddress,
        contract_name: &str,
        function_name: &str,
        args: Vec<Value>,
    ) -> Option<TransactionPayload> {
        let contract_name_str = match ContractName::try_from(contract_name.to_string()) {
            Ok(s) => s,
            Err(_) => {
                wrb_test_debug!("Not a clarity name: '{}'", contract_name);
                return None;
            }
        };

        let function_name_str = match ClarityName::try_from(function_name.to_string()) {
            Ok(s) => s,
            Err(_) => {
                wrb_test_debug!("Not a clarity name: '{}'", contract_name);
                return None;
            }
        };

        Some(TransactionPayload::ContractCall(TransactionContractCall {
            address: contract_address,
            contract_name: contract_name_str,
            function_name: function_name_str,
            function_args: args,
        }))
    }

    pub fn new_smart_contract(
        name: &str,
        contract: &str,
        version_opt: Option<ClarityVersion>,
    ) -> Option<TransactionPayload> {
        match (
            ContractName::try_from(name.to_string()),
            StacksString::from_str(contract),
        ) {
            (Ok(s_name), Some(s_body)) => Some(TransactionPayload::SmartContract(
                TransactionSmartContract {
                    name: s_name,
                    code_body: s_body,
                },
                version_opt,
            )),
            (_, _) => None,
        }
    }
}

impl StacksMessageCodec for AssetInfo {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), codec_error> {
        write_next(fd, &self.contract_address)?;
        write_next(fd, &self.contract_name)?;
        write_next(fd, &self.asset_name)?;
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<AssetInfo, codec_error> {
        let contract_address: StacksAddress = read_next(fd)?;
        let contract_name: ContractName = read_next(fd)?;
        let asset_name: ClarityName = read_next(fd)?;
        Ok(AssetInfo {
            contract_address,
            contract_name,
            asset_name,
        })
    }
}

impl StacksMessageCodec for PostConditionPrincipal {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), codec_error> {
        match *self {
            PostConditionPrincipal::Origin => {
                write_next(fd, &(PostConditionPrincipalID::Origin as u8))?;
            }
            PostConditionPrincipal::Standard(ref address) => {
                write_next(fd, &(PostConditionPrincipalID::Standard as u8))?;
                write_next(fd, address)?;
            }
            PostConditionPrincipal::Contract(ref address, ref contract_name) => {
                write_next(fd, &(PostConditionPrincipalID::Contract as u8))?;
                write_next(fd, address)?;
                write_next(fd, contract_name)?;
            }
        }
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<PostConditionPrincipal, codec_error> {
        let principal_id: u8 = read_next(fd)?;
        let principal = match principal_id {
            x if x == PostConditionPrincipalID::Origin as u8 => PostConditionPrincipal::Origin,
            x if x == PostConditionPrincipalID::Standard as u8 => {
                let addr: StacksAddress = read_next(fd)?;
                PostConditionPrincipal::Standard(addr)
            }
            x if x == PostConditionPrincipalID::Contract as u8 => {
                let addr: StacksAddress = read_next(fd)?;
                let contract_name: ContractName = read_next(fd)?;
                PostConditionPrincipal::Contract(addr, contract_name)
            }
            _ => {
                return Err(codec_error::DeserializeError(format!(
                    "Failed to parse transaction: unknown post condition principal ID {}",
                    principal_id
                )));
            }
        };
        Ok(principal)
    }
}

impl StacksMessageCodec for TransactionPostCondition {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), codec_error> {
        match *self {
            TransactionPostCondition::STX(ref principal, ref fungible_condition, ref amount) => {
                write_next(fd, &(AssetInfoID::STX as u8))?;
                write_next(fd, principal)?;
                write_next(fd, &(*fungible_condition as u8))?;
                write_next(fd, amount)?;
            }
            TransactionPostCondition::Fungible(
                ref principal,
                ref asset_info,
                ref fungible_condition,
                ref amount,
            ) => {
                write_next(fd, &(AssetInfoID::FungibleAsset as u8))?;
                write_next(fd, principal)?;
                write_next(fd, asset_info)?;
                write_next(fd, &(*fungible_condition as u8))?;
                write_next(fd, amount)?;
            }
            TransactionPostCondition::Nonfungible(
                ref principal,
                ref asset_info,
                ref asset_value,
                ref nonfungible_condition,
            ) => {
                write_next(fd, &(AssetInfoID::NonfungibleAsset as u8))?;
                write_next(fd, principal)?;
                write_next(fd, asset_info)?;
                write_next(fd, asset_value)?;
                write_next(fd, &(*nonfungible_condition as u8))?;
            }
        };
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<TransactionPostCondition, codec_error> {
        let asset_info_id: u8 = read_next(fd)?;
        let postcond = match asset_info_id {
            x if x == AssetInfoID::STX as u8 => {
                let principal: PostConditionPrincipal = read_next(fd)?;
                let condition_u8: u8 = read_next(fd)?;
                let amount: u64 = read_next(fd)?;

                let condition_code = FungibleConditionCode::from_u8(condition_u8).ok_or(
                    codec_error::DeserializeError(format!(
                    "Failed to parse transaction: Failed to parse STX fungible condition code {}",
                    condition_u8
                )),
                )?;

                TransactionPostCondition::STX(principal, condition_code, amount)
            }
            x if x == AssetInfoID::FungibleAsset as u8 => {
                let principal: PostConditionPrincipal = read_next(fd)?;
                let asset: AssetInfo = read_next(fd)?;
                let condition_u8: u8 = read_next(fd)?;
                let amount: u64 = read_next(fd)?;

                let condition_code = FungibleConditionCode::from_u8(condition_u8).ok_or(
                    codec_error::DeserializeError(format!(
                    "Failed to parse transaction: Failed to parse FungibleAsset condition code {}",
                    condition_u8
                )),
                )?;

                TransactionPostCondition::Fungible(principal, asset, condition_code, amount)
            }
            x if x == AssetInfoID::NonfungibleAsset as u8 => {
                let principal: PostConditionPrincipal = read_next(fd)?;
                let asset: AssetInfo = read_next(fd)?;
                let asset_value: Value = read_next(fd)?;
                let condition_u8: u8 = read_next(fd)?;

                let condition_code = NonfungibleConditionCode::from_u8(condition_u8)
                    .ok_or(codec_error::DeserializeError(format!("Failed to parse transaction: Failed to parse NonfungibleAsset condition code {}", condition_u8)))?;

                TransactionPostCondition::Nonfungible(principal, asset, asset_value, condition_code)
            }
            _ => {
                return Err(codec_error::DeserializeError(format!(
                    "Failed to aprse transaction: unknown asset info ID {}",
                    asset_info_id
                )));
            }
        };

        Ok(postcond)
    }
}

impl StacksTransaction {
    pub fn tx_len(&self) -> u64 {
        let mut tx_bytes = vec![];
        self.consensus_serialize(&mut tx_bytes)
            .expect("BUG: Failed to serialize a transaction object");
        u64::try_from(tx_bytes.len()).expect("tx len exceeds 2^64 bytes")
    }

    pub fn consensus_deserialize_with_len<R: Read>(
        fd: &mut R,
    ) -> Result<(StacksTransaction, u64), codec_error> {
        let mut bound_read = BoundReader::from_reader(fd, MAX_TRANSACTION_LEN.into());
        let fd = &mut bound_read;

        let version_u8: u8 = read_next(fd)?;
        let chain_id: u32 = read_next(fd)?;
        let auth: TransactionAuth = read_next(fd)?;
        let anchor_mode_u8: u8 = read_next(fd)?;
        let post_condition_mode_u8: u8 = read_next(fd)?;
        let post_conditions: Vec<TransactionPostCondition> = read_next(fd)?;

        let payload: TransactionPayload = read_next(fd)?;

        let version = if (version_u8 & 0x80) == 0 {
            TransactionVersion::Mainnet
        } else {
            TransactionVersion::Testnet
        };

        let anchor_mode = match anchor_mode_u8 {
            x if x == TransactionAnchorMode::OffChainOnly as u8 => {
                TransactionAnchorMode::OffChainOnly
            }
            x if x == TransactionAnchorMode::OnChainOnly as u8 => {
                TransactionAnchorMode::OnChainOnly
            }
            x if x == TransactionAnchorMode::Any as u8 => TransactionAnchorMode::Any,
            _ => {
                return Err(codec_error::DeserializeError(format!(
                    "Failed to parse transaction: invalid anchor mode {}",
                    anchor_mode_u8
                )));
            }
        };

        let post_condition_mode = match post_condition_mode_u8 {
            x if x == TransactionPostConditionMode::Allow as u8 => {
                TransactionPostConditionMode::Allow
            }
            x if x == TransactionPostConditionMode::Deny as u8 => {
                TransactionPostConditionMode::Deny
            }
            _ => {
                return Err(codec_error::DeserializeError(format!(
                    "Failed to parse transaction: invalid post-condition mode {}",
                    post_condition_mode_u8
                )));
            }
        };
        let tx = StacksTransaction {
            version,
            chain_id,
            auth,
            anchor_mode,
            post_condition_mode,
            post_conditions,
            payload,
        };

        Ok((tx, fd.num_read()))
    }
}

impl StacksMessageCodec for StacksTransaction {
    fn consensus_serialize<W: Write>(&self, fd: &mut W) -> Result<(), codec_error> {
        write_next(fd, &(self.version as u8))?;
        write_next(fd, &self.chain_id)?;
        write_next(fd, &self.auth)?;
        write_next(fd, &(self.anchor_mode as u8))?;
        write_next(fd, &(self.post_condition_mode as u8))?;
        write_next(fd, &self.post_conditions)?;
        write_next(fd, &self.payload)?;
        Ok(())
    }

    fn consensus_deserialize<R: Read>(fd: &mut R) -> Result<StacksTransaction, codec_error> {
        StacksTransaction::consensus_deserialize_with_len(fd).map(|(result, _)| result)
    }
}

impl From<TransactionSmartContract> for TransactionPayload {
    fn from(value: TransactionSmartContract) -> Self {
        TransactionPayload::SmartContract(value, None)
    }
}

impl From<TransactionContractCall> for TransactionPayload {
    fn from(value: TransactionContractCall) -> Self {
        TransactionPayload::ContractCall(value)
    }
}

impl StacksTransaction {
    /// Create a new, unsigned transaction and an empty STX fee with no post-conditions.
    pub fn new(
        version: TransactionVersion,
        auth: TransactionAuth,
        payload: TransactionPayload,
    ) -> StacksTransaction {
        let anchor_mode = TransactionAnchorMode::Any;
        StacksTransaction {
            version,
            chain_id: 0,
            auth,
            anchor_mode,
            post_condition_mode: TransactionPostConditionMode::Deny,
            post_conditions: vec![],
            payload,
        }
    }

    /// Get fee rate
    pub fn get_tx_fee(&self) -> u64 {
        self.auth.get_tx_fee()
    }

    /// Set fee rate
    pub fn set_tx_fee(&mut self, tx_fee: u64) {
        self.auth.set_tx_fee(tx_fee);
    }

    /// Get origin nonce
    pub fn get_origin_nonce(&self) -> u64 {
        self.auth.get_origin_nonce()
    }

    /// get sponsor nonce
    pub fn get_sponsor_nonce(&self) -> Option<u64> {
        self.auth.get_sponsor_nonce()
    }

    /// set origin nonce
    pub fn set_origin_nonce(&mut self, n: u64) {
        self.auth.set_origin_nonce(n);
    }

    /// set sponsor nonce
    pub fn set_sponsor_nonce(&mut self, n: u64) -> Result<(), Error> {
        self.auth.set_sponsor_nonce(n)
    }

    /// Set anchor mode
    pub fn set_anchor_mode(&mut self, anchor_mode: TransactionAnchorMode) {
        self.anchor_mode = anchor_mode;
    }

    /// Set post-condition mode
    pub fn set_post_condition_mode(&mut self, postcond_mode: TransactionPostConditionMode) {
        self.post_condition_mode = postcond_mode;
    }

    /// Add a post-condition
    pub fn add_post_condition(&mut self, post_condition: TransactionPostCondition) {
        self.post_conditions.push(post_condition);
    }

    /// a txid of a stacks transaction is its sha512/256 hash
    pub fn txid(&self) -> Txid {
        let mut bytes = vec![];
        self.consensus_serialize(&mut bytes)
            .expect("BUG: failed to serialize to a vec");
        Txid::from_stacks_tx(&bytes)
    }

    /// Get a mutable reference to the internal auth structure
    pub fn borrow_auth(&mut self) -> &mut TransactionAuth {
        &mut self.auth
    }

    /// Get an immutable reference to the internal auth structure
    pub fn auth(&self) -> &TransactionAuth {
        &self.auth
    }

    /// begin signing the transaction.
    /// If this is a sponsored transaction, then the origin only commits to knowing that it is
    /// sponsored.  It does _not_ commit to the sponsored fields, so set them all to sentinel
    /// values.
    /// Return the initial sighash.
    fn sign_begin(&self) -> Txid {
        let mut tx = self.clone();
        tx.auth = tx.auth.into_initial_sighash_auth();
        tx.txid()
    }

    /// begin verifying a transaction.
    /// return the initial sighash
    fn verify_begin(&self) -> Txid {
        let mut tx = self.clone();
        tx.auth = tx.auth.into_initial_sighash_auth();
        tx.txid()
    }

    /// Sign a sighash and append the signature and public key to the given spending condition.
    /// Returns the next sighash
    fn sign_and_append(
        condition: &mut TransactionSpendingCondition,
        cur_sighash: &Txid,
        auth_flag: &TransactionAuthFlags,
        privk: &StacksPrivateKey,
    ) -> Result<Txid, Error> {
        let (next_sig, next_sighash) = TransactionSpendingCondition::next_signature(
            cur_sighash,
            auth_flag,
            condition.tx_fee(),
            condition.nonce(),
            privk,
        )?;
        match condition {
            TransactionSpendingCondition::Singlesig(ref mut cond) => {
                cond.set_signature(next_sig);
                Ok(next_sighash)
            }
            TransactionSpendingCondition::Multisig(ref mut cond) => {
                cond.push_signature(
                    if privk.compress_public() {
                        TransactionPublicKeyEncoding::Compressed
                    } else {
                        TransactionPublicKeyEncoding::Uncompressed
                    },
                    next_sig,
                );
                Ok(next_sighash)
            }
            TransactionSpendingCondition::OrderIndependentMultisig(ref mut cond) => {
                cond.push_signature(
                    if privk.compress_public() {
                        TransactionPublicKeyEncoding::Compressed
                    } else {
                        TransactionPublicKeyEncoding::Uncompressed
                    },
                    next_sig,
                );
                Ok(*cur_sighash)
            }
        }
    }

    /// Pop the last auth field
    fn pop_auth_field(
        condition: &mut TransactionSpendingCondition,
    ) -> Option<TransactionAuthField> {
        match condition {
            TransactionSpendingCondition::Multisig(ref mut cond) => cond.pop_auth_field(),
            TransactionSpendingCondition::OrderIndependentMultisig(ref mut cond) => {
                cond.pop_auth_field()
            }
            TransactionSpendingCondition::Singlesig(ref mut cond) => cond.pop_signature(),
        }
    }

    /// Append a public key to a multisig condition
    fn append_pubkey(
        condition: &mut TransactionSpendingCondition,
        pubkey: &StacksPublicKey,
    ) -> Result<(), Error> {
        match condition {
            TransactionSpendingCondition::Multisig(ref mut cond) => {
                cond.push_public_key(pubkey.clone());
                Ok(())
            }
            TransactionSpendingCondition::OrderIndependentMultisig(ref mut cond) => {
                cond.push_public_key(pubkey.clone());
                Ok(())
            }
            _ => Err(Error::SigningError("Not a multisig condition".to_string())),
        }
    }

    /// Append the next signature from the origin account authorization.
    /// Return the next sighash.
    pub fn sign_next_origin(
        &mut self,
        cur_sighash: &Txid,
        privk: &StacksPrivateKey,
    ) -> Result<Txid, Error> {
        let next_sighash = match self.auth {
            TransactionAuth::Standard(ref mut origin_condition)
            | TransactionAuth::Sponsored(ref mut origin_condition, _) => {
                StacksTransaction::sign_and_append(
                    origin_condition,
                    cur_sighash,
                    &TransactionAuthFlags::AuthStandard,
                    privk,
                )?
            }
        };
        Ok(next_sighash)
    }

    /// Append the next public key to the origin account authorization.
    pub fn append_next_origin(&mut self, pubk: &StacksPublicKey) -> Result<(), Error> {
        match self.auth {
            TransactionAuth::Standard(ref mut origin_condition) => {
                StacksTransaction::append_pubkey(origin_condition, pubk)
            }
            TransactionAuth::Sponsored(ref mut origin_condition, _) => {
                StacksTransaction::append_pubkey(origin_condition, pubk)
            }
        }
    }

    /// Append the next signature from the sponsoring account.
    /// Return the next sighash
    pub fn sign_next_sponsor(
        &mut self,
        cur_sighash: &Txid,
        privk: &StacksPrivateKey,
    ) -> Result<Txid, Error> {
        let next_sighash = match self.auth {
            TransactionAuth::Standard(_) => {
                // invalid
                return Err(Error::SigningError(
                    "Cannot sign standard authorization with a sponsoring private key".to_string(),
                ));
            }
            TransactionAuth::Sponsored(_, ref mut sponsor_condition) => {
                StacksTransaction::sign_and_append(
                    sponsor_condition,
                    cur_sighash,
                    &TransactionAuthFlags::AuthSponsored,
                    privk,
                )?
            }
        };
        Ok(next_sighash)
    }

    /// Append the next public key to the sponsor account authorization.
    pub fn append_next_sponsor(&mut self, pubk: &StacksPublicKey) -> Result<(), Error> {
        match self.auth {
            TransactionAuth::Standard(_) => Err(Error::SigningError(
                "Cannot appned a public key to the sponsor of a standard auth condition"
                    .to_string(),
            )),
            TransactionAuth::Sponsored(_, ref mut sponsor_condition) => {
                StacksTransaction::append_pubkey(sponsor_condition, pubk)
            }
        }
    }

    /// Verify this transaction's signatures
    pub fn verify(&self) -> Result<(), Error> {
        self.auth.verify(&self.verify_begin())
    }

    /// Verify the transaction's origin signatures only.
    /// Used by sponsors to get the next sig-hash to sign.
    pub fn verify_origin(&self) -> Result<Txid, Error> {
        self.auth.verify_origin(&self.verify_begin())
    }

    /// Get the origin account's address
    pub fn origin_address(&self) -> StacksAddress {
        match (&self.version, &self.auth) {
            (TransactionVersion::Mainnet, TransactionAuth::Standard(origin_condition)) => {
                origin_condition.address_mainnet()
            }
            (TransactionVersion::Testnet, TransactionAuth::Standard(origin_condition)) => {
                origin_condition.address_testnet()
            }
            (
                TransactionVersion::Mainnet,
                TransactionAuth::Sponsored(origin_condition, _unused),
            ) => origin_condition.address_mainnet(),
            (
                TransactionVersion::Testnet,
                TransactionAuth::Sponsored(origin_condition, _unused),
            ) => origin_condition.address_testnet(),
        }
    }

    /// Get the sponsor account's address, if this transaction is sponsored
    pub fn sponsor_address(&self) -> Option<StacksAddress> {
        match (&self.version, &self.auth) {
            (TransactionVersion::Mainnet, TransactionAuth::Standard(_unused)) => None,
            (TransactionVersion::Testnet, TransactionAuth::Standard(_unused)) => None,
            (
                TransactionVersion::Mainnet,
                TransactionAuth::Sponsored(_unused, sponsor_condition),
            ) => Some(sponsor_condition.address_mainnet()),
            (
                TransactionVersion::Testnet,
                TransactionAuth::Sponsored(_unused, sponsor_condition),
            ) => Some(sponsor_condition.address_testnet()),
        }
    }

    /// Get a copy of the origin spending condition
    pub fn get_origin(&self) -> TransactionSpendingCondition {
        self.auth.origin().clone()
    }

    /// Get a copy of the sending condition that will pay the tx fee
    pub fn get_payer(&self) -> TransactionSpendingCondition {
        match self.auth.sponsor() {
            Some(ref tsc) => (*tsc).clone(),
            None => self.auth.origin().clone(),
        }
    }

    /// Is this a mainnet transaction?  false means 'testnet'
    pub fn is_mainnet(&self) -> bool {
        match self.version {
            TransactionVersion::Mainnet => true,
            _ => false,
        }
    }
}

impl StacksTransactionSigner {
    pub fn new(tx: &StacksTransaction) -> StacksTransactionSigner {
        StacksTransactionSigner {
            tx: tx.clone(),
            sighash: tx.sign_begin(),
            origin_done: false,
            check_oversign: true,
            check_overlap: true,
        }
    }

    pub fn new_sponsor(
        tx: &StacksTransaction,
        spending_condition: TransactionSpendingCondition,
    ) -> Result<StacksTransactionSigner, Error> {
        if !tx.auth.is_sponsored() {
            return Err(Error::IncompatibleSpendingConditionError);
        }
        let mut new_tx = tx.clone();
        new_tx.auth.set_sponsor(spending_condition)?;
        let origin_sighash = new_tx.verify_origin()?;

        Ok(StacksTransactionSigner {
            tx: new_tx,
            sighash: origin_sighash,
            origin_done: true,
            check_oversign: true,
            check_overlap: true,
        })
    }

    pub fn resume(&mut self, tx: &StacksTransaction) {
        self.tx = tx.clone()
    }

    pub fn disable_checks(&mut self) {
        self.check_oversign = false;
        self.check_overlap = false;
    }

    pub fn sign_origin(&mut self, privk: &StacksPrivateKey) -> Result<(), Error> {
        if self.check_overlap && self.origin_done {
            // can't sign another origin private key since we started signing sponsors
            return Err(Error::SigningError(
                "Cannot sign origin after sponsor key".to_string(),
            ));
        }

        match self.tx.auth {
            TransactionAuth::Standard(ref origin_condition) => {
                if self.check_oversign
                    && origin_condition.num_signatures() >= origin_condition.signatures_required()
                {
                    return Err(Error::SigningError(
                        "Origin would have too many signatures".to_string(),
                    ));
                }
            }
            TransactionAuth::Sponsored(ref origin_condition, _) => {
                if self.check_oversign
                    && origin_condition.num_signatures() >= origin_condition.signatures_required()
                {
                    return Err(Error::SigningError(
                        "Origin would have too many signatures".to_string(),
                    ));
                }
            }
        }

        let next_sighash = self.tx.sign_next_origin(&self.sighash, privk)?;
        self.sighash = next_sighash;
        Ok(())
    }

    pub fn append_origin(&mut self, pubk: &StacksPublicKey) -> Result<(), Error> {
        if self.check_overlap && self.origin_done {
            // can't append another origin key
            return Err(Error::SigningError(
                "Cannot append public key to origin after sponsor key".to_string(),
            ));
        }

        self.tx.append_next_origin(pubk)
    }

    pub fn sign_sponsor(&mut self, privk: &StacksPrivateKey) -> Result<(), Error> {
        match self.tx.auth {
            TransactionAuth::Sponsored(_, ref sponsor_condition) => {
                if self.check_oversign
                    && sponsor_condition.num_signatures() >= sponsor_condition.signatures_required()
                {
                    return Err(Error::SigningError(
                        "Sponsor would have too many signatures".to_string(),
                    ));
                }
            }
            _ => {}
        }

        let next_sighash = self.tx.sign_next_sponsor(&self.sighash, privk)?;
        self.sighash = next_sighash;
        self.origin_done = true;
        Ok(())
    }

    pub fn append_sponsor(&mut self, pubk: &StacksPublicKey) -> Result<(), Error> {
        self.tx.append_next_sponsor(pubk)
    }

    pub fn pop_origin_auth_field(&mut self) -> Option<TransactionAuthField> {
        match self.tx.auth {
            TransactionAuth::Standard(ref mut origin_condition) => {
                StacksTransaction::pop_auth_field(origin_condition)
            }
            TransactionAuth::Sponsored(ref mut origin_condition, _) => {
                StacksTransaction::pop_auth_field(origin_condition)
            }
        }
    }

    pub fn pop_sponsor_auth_field(&mut self) -> Option<TransactionAuthField> {
        match self.tx.auth {
            TransactionAuth::Sponsored(_, ref mut sponsor_condition) => {
                StacksTransaction::pop_auth_field(sponsor_condition)
            }
            _ => None,
        }
    }

    pub fn complete(&self) -> bool {
        match self.tx.auth {
            TransactionAuth::Standard(ref origin_condition) => {
                origin_condition.num_signatures() >= origin_condition.signatures_required()
            }
            TransactionAuth::Sponsored(ref origin_condition, ref sponsored_condition) => {
                origin_condition.num_signatures() >= origin_condition.signatures_required()
                    && sponsored_condition.num_signatures()
                        >= sponsored_condition.signatures_required()
                    && (self.origin_done || !self.check_overlap)
            }
        }
    }

    pub fn get_tx_incomplete(&self) -> StacksTransaction {
        self.tx.clone()
    }

    pub fn get_tx(&self) -> Option<StacksTransaction> {
        if self.complete() {
            Some(self.tx.clone())
        } else {
            None
        }
    }
}

// N.B. tests are handled by stacks-core. This code is lifted verbatim.
