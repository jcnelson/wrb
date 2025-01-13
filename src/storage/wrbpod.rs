// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2023 Stacks Open Internet Foundation
// Copyright (C) 2023 Jude Nelson
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

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::storage::{
    Error, StackerDBClient, Wrbpod, WrbpodAppState, WrbpodSlices, WrbpodSuperblock,
    WRBPOD_APP_STATE_VERSION, WRBPOD_CHUNK_MAX_SIZE, WRBPOD_MAX_SLOTS, WRBPOD_SLICES_VERSION,
    WRBPOD_SUPERBLOCK_SLOT_ID, WRBPOD_SUPERBLOCK_VERSION,
};

use clarity::vm::types::QualifiedContractIdentifier;

use stacks_common::codec::Error as CodecError;
use stacks_common::codec::StacksMessageCodec;
use stacks_common::types::chainstate::StacksAddress;
use stacks_common::types::chainstate::StacksPublicKey;
use stacks_common::util::hash::Hash160;
use stacks_common::util::hash::Sha512Trunc256Sum;
use stacks_common::util::secp256k1::Secp256k1PrivateKey;

use libstackerdb::SlotMetadata;
use libstackerdb::StackerDBChunkData;

pub const WRBPOD_SLICES_INITIAL_SIZE: u64 = 1 + 8; // version + index_length

pub const SIGNER_REFRESH_INTERVAL: u64 = 60; // refresh once every 60 seconds

impl WrbpodSlices {
    pub fn new() -> Self {
        Self {
            version: WRBPOD_SLICES_VERSION,
            slices: vec![],
            index: BTreeMap::new(),
            dirty: false,
            encoded_size: WRBPOD_SLICES_INITIAL_SIZE,
            max_size: WRBPOD_CHUNK_MAX_SIZE.into(),
        }
    }

    /// what's the encoded size of an additional slice?
    pub(crate) fn slice_encoded_size(slice_len: usize, present: bool) -> u64 {
        let sz: u64 = 4 + (u64::try_from(slice_len).expect("slice too big"));
        if present {
            return sz;
        }
        // account for the extra index state:
        // - 16-byte wrb ID
        // - 8-byte index
        sz + 16 + 8
    }

    /// Can a slice of a given length fit?
    pub fn can_fit_slice(&self, id: u128, size: usize) -> bool {
        self.encoded_size + Self::slice_encoded_size(size, self.index.contains_key(&id))
            <= self.max_size
    }

    /// Add or replace a slice
    /// Return true if inserted
    /// Return false if not inserted (e.g. space exceeded)
    pub fn put_slice(&mut self, id: u128, slice: Vec<u8>) -> bool {
        if let Some(idx) = self.index.get(&id) {
            if self.encoded_size + Self::slice_encoded_size(slice.len(), true) > self.max_size {
                return false;
            }
            self.encoded_size += Self::slice_encoded_size(slice.len(), true);
            self.slices[*idx] = slice;
        } else {
            if self.encoded_size + Self::slice_encoded_size(slice.len(), false) > self.max_size {
                return false;
            }
            self.encoded_size += Self::slice_encoded_size(slice.len(), false);
            self.index.insert(id, self.slices.len());
            self.slices.push(slice);
        }
        self.dirty = true;
        true
    }

    /// Get a slice by ID
    pub fn get_slice(&self, id: u128) -> Option<&Vec<u8>> {
        let Some(idx) = self.index.get(&id) else {
            return None;
        };
        self.slices.get(*idx)
    }

    /// Convert to an unsigned StackerDBChunkData
    pub fn to_stackerdb_chunk(&self, slot_id: u32, slot_version: u32) -> StackerDBChunkData {
        let bytes = self.serialize_to_vec();
        StackerDBChunkData::new(slot_id, slot_version, bytes)
    }

    /// Load from a StackerDBChunkData
    pub fn from_slice(mut data: &[u8]) -> Result<Self, CodecError> {
        WrbpodSlices::consensus_deserialize(&mut data)
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }
}

impl WrbpodSuperblock {
    pub fn new() -> Self {
        Self {
            version: WRBPOD_SUPERBLOCK_VERSION,
            apps: BTreeMap::new(),
        }
    }

    /// Find a free slot
    fn find_free_slot(&self, used: &[u32]) -> Option<u32> {
        let mut occupied = HashSet::new();
        for app_state in self.apps.values() {
            for slot in app_state.slots.iter() {
                occupied.insert(*slot);
            }
        }
        for s in used {
            occupied.insert(*s);
        }

        // NOTE: slot 0 is the superblock
        for slot in 1..WRBPOD_MAX_SLOTS {
            if !occupied.contains(&slot) {
                return Some(slot);
            }
        }

        return None;
    }

    /// Allocate more slots to a particular app.
    /// If there's no app state for this app, then make it.
    /// Returns true if we could allocate slots.
    /// Returns false if there's not enough space.
    pub fn allocate_slots(&mut self, app_name: &str, code_hash: Hash160, num_slots: u32) -> bool {
        // find more slots
        let mut slots = vec![];
        for _i in 0..num_slots {
            let Some(free_slot) = self.find_free_slot(&slots) else {
                // not enough space
                wrb_test_debug!(
                    "Not enough free space to allocate {} slots for {} ({})",
                    num_slots,
                    app_name,
                    &code_hash
                );
                return false;
            };
            slots.push(free_slot);
        }

        let Some(app_state) = self.apps.get_mut(&app_name.to_string()) else {
            let new_app_state = WrbpodAppState {
                version: WRBPOD_APP_STATE_VERSION,
                code_hash,
                slots,
            };
            self.apps.insert(app_name.to_string(), new_app_state);
            wrb_test_debug!(
                "Added {} more slots for existing app state for {} ({})",
                num_slots,
                app_name,
                &code_hash
            );
            return true;
        };

        // add slots
        app_state.slots.append(&mut slots);
        wrb_test_debug!(
            "Instantiated {} slots for new app state for {} ({})",
            num_slots,
            app_name,
            &code_hash
        );
        return true;
    }

    /// Free up slots for an app. Drops all of its app state
    pub fn delete_slots(&mut self, app_name: &str) {
        self.apps.remove(&app_name.to_string());
    }

    /// Get a ref to an app's state
    pub fn app_state(&self, app_name: &str) -> Option<&WrbpodAppState> {
        self.apps.get(&app_name.to_string())
    }

    pub fn num_app_slots(&self, app_name: &str) -> u32 {
        if let Some(app_state) = self.app_state(app_name) {
            u32::try_from(app_state.slots.len()).expect("infallible")
        } else {
            0
        }
    }

    /// Convert an application slot ID to a stackerdb chunk ID.
    /// Slots are logical chunks -- an application's slots are numbered 0..NUM_SLOTS,
    /// there are multiple apps that share the stackerdb's chunks.
    fn app_slot_id_to_stackerdb_chunk_id(&self, app_name: &str, app_slot_id: u32) -> Option<u32> {
        let Some(app_state) = self.apps.get(&app_name.to_string()) else {
            return None;
        };
        let Ok(app_slot_idx) = usize::try_from(app_slot_id) else {
            return None;
        };
        app_state.slots.get(app_slot_idx).map(|chunk_id| *chunk_id)
    }
}

impl Wrbpod {
    /// open an existing wrbpod
    pub fn open(
        home_client: Box<dyn StackerDBClient>,
        replica_client: Box<dyn StackerDBClient>,
        privkey: Secp256k1PrivateKey,
    ) -> Result<Self, Error> {
        let mut wrbpod = Wrbpod {
            superblock: WrbpodSuperblock::new(),
            privkey,
            home_client,
            replica_client,
            chunks: HashMap::new(),
            signers: None,
        };
        wrbpod.refresh_signers()?;
        wrbpod.download_superblock()?;
        Ok(wrbpod)
    }

    /// create a new wrbpod with an empty superblock
    pub fn format(
        home_client: Box<dyn StackerDBClient>,
        replica_client: Box<dyn StackerDBClient>,
        privkey: Secp256k1PrivateKey,
    ) -> Result<Self, Error> {
        let mut wrbpod = Wrbpod {
            superblock: WrbpodSuperblock::new(),
            privkey,
            home_client,
            replica_client,
            chunks: HashMap::new(),
            signers: None,
        };
        wrbpod.refresh_signers()?;
        wrbpod.upload_superblock()?;
        Ok(wrbpod)
    }

    /// Update the list of signers.
    /// We ask the *home client* for this, since it's trusted.
    fn refresh_signers(&mut self) -> Result<(), Error> {
        let signers = self.home_client.get_signers()?;
        self.signers = Some(signers);
        Ok(())
    }

    /// Update the cached copy of the superblock
    fn download_superblock(&mut self) -> Result<(), Error> {
        let all_slot_metadata = self.replica_client.list_chunks()?;
        let slot_md = all_slot_metadata.get(0).ok_or(Error::GetChunk(
            "no superblock chunk defined in slot metadata".into(),
        ))?;

        if self.signers.is_none() {
            self.refresh_signers()?;
        }

        let Some(signers) = self.signers.as_ref() else {
            return Err(Error::GetChunk("Unable to load signer list".into()));
        };
        let Some(signer_addr) = signers.get(0).cloned() else {
            return Err(Error::GetChunk(format!(
                "No such signer for chunk ID {}",
                0
            )));
        };

        if slot_md.slot_version == 0 && slot_md.data_hash == Sha512Trunc256Sum([0x00; 32]) {
            // no superblock instantiated yet
            self.superblock = WrbpodSuperblock::new();
            return Ok(());
        }

        if !slot_md.verify(&signer_addr).map_err(|e| {
            Error::GetChunk(format!(
                "Failed to verify signature by {} on {:?}: {:?}",
                &signer_addr, &slot_md, &e
            ))
        })? {
            wrb_warn!(
                "Superblock slot is not signed by signer; signer_addr = {}, metadata = {:?}",
                &signer_addr,
                &slot_md
            );
            return Err(Error::GetChunk("Invalid superblock signature".into()));
        }

        // get the superblock chunk
        let chunks = self.replica_client.get_latest_chunks(&[0])?;
        let Some(chunk_opt) = chunks.get(0) else {
            return Err(Error::NoSuchChunk);
        };
        let Some(chunk) = chunk_opt else {
            return Err(Error::NoSuchChunk);
        };
        if slot_md.data_hash != Sha512Trunc256Sum::from_data(&chunk) {
            return Err(Error::GetChunk("superblock chunk hash mismatch".into()));
        }

        let superblock = WrbpodSuperblock::consensus_deserialize(&mut &chunk[..])?;
        self.superblock = superblock;
        Ok(())
    }

    /// Save the superblock.
    /// Retries on stale version.
    fn upload_superblock(&mut self) -> Result<(), Error> {
        let slot_metadata = self.replica_client.list_chunks()?;
        let superblock_md = slot_metadata
            .get(WRBPOD_SUPERBLOCK_SLOT_ID as usize)
            .ok_or(Error::PutChunk(
                "No superblock chunk defined in slot metadata".into(),
            ))?;
        let superblock_bytes = self.superblock.serialize_to_vec();
        let mut slot_version = superblock_md.slot_version;
        loop {
            let mut superblock_chunk = StackerDBChunkData::new(
                WRBPOD_SUPERBLOCK_SLOT_ID,
                slot_version,
                superblock_bytes.clone(),
            );
            superblock_chunk
                .sign(&self.privkey)
                .map_err(|_| Error::Codec(CodecError::SerializeError("Failed to sign".into())))?;

            wrb_test_debug!(
                "Signed superblock with {} ({}): {:?}",
                &self.privkey.to_hex(),
                StacksAddress::p2pkh(true, &StacksPublicKey::from_private(&self.privkey)),
                &superblock_chunk
            );

            let result = self.replica_client.put_chunk(superblock_chunk)?;
            if result.accepted {
                break;
            }

            let reason = result.reason.unwrap_or("(reason not given)".to_string());
            wrb_warn!("Failed to save superblock: reason was '{}'", &reason);

            if let Some(metadata) = result.metadata {
                // newer version
                slot_version = metadata.slot_version + 1;
                continue;
            }

            return Err(Error::PutChunk(reason));
        }
        Ok(())
    }

    /// Get a ref to the superblock
    pub fn superblock(&self) -> &WrbpodSuperblock {
        &self.superblock
    }

    /// Allocate slots in the superblock for an app.
    /// If the app doesn't exist, then create state for it.
    /// Returns Ok(true) if we succeed
    /// Returns Ok(false) if there's not enough space
    /// Returns Err(..) on network error
    pub fn allocate_slots(
        &mut self,
        app_name: &str,
        code_hash: Hash160,
        num_slots: u32,
    ) -> Result<bool, Error> {
        self.download_superblock()?;
        let success = self
            .superblock
            .allocate_slots(app_name, code_hash, num_slots);
        if success {
            self.upload_superblock()?;
        }
        Ok(success)
    }

    /// Get the number of allocated slots for the app
    pub fn get_num_slots(&self, app_name: &str) -> u64 {
        self.superblock.num_app_slots(app_name).into()
    }

    /// Delete app state from the superblock.
    /// Returns Ok(()) if we succeed
    /// Returns Err(..) on network error
    pub fn delete_slots(&mut self, app_name: &str) -> Result<(), Error> {
        self.download_superblock()?;
        self.superblock.delete_slots(app_name);
        self.upload_superblock()?;
        Ok(())
    }

    /// Save a chunk directly.  Used for low-level things, like manually patching the wrbpod.
    pub(crate) fn put_chunk(&mut self, mut chunk: StackerDBChunkData) -> Result<(), Error> {
        loop {
            chunk
                .sign(&self.privkey)
                .map_err(|_| Error::Codec(CodecError::SerializeError("Failed to sign".into())))?;
            wrb_test_debug!(
                "Signed with {} ({}): {:?}",
                &self.privkey.to_hex(),
                StacksAddress::p2pkh(true, &StacksPublicKey::from_private(&self.privkey)),
                &chunk
            );

            let result = self.replica_client.put_chunk(chunk.clone())?;
            if result.accepted {
                break;
            }

            let reason = result.reason.unwrap_or("(reason not given)".to_string());
            wrb_warn!(
                "Failed to save chunk ({},{}): reason was '{}'",
                chunk.slot_id,
                chunk.slot_version,
                &reason
            );

            if let Some(metadata) = result.metadata {
                // newer version
                chunk.slot_version = metadata.slot_version + 1;
                continue;
            }

            return Err(Error::PutChunk(reason));
        }
        Ok(())
    }

    /// List all chunks in the StackerDB.  Used for low-level things like manually patchin the
    /// wrbpod.
    pub(crate) fn list_chunks(&mut self) -> Result<Vec<SlotMetadata>, Error> {
        Ok(self.replica_client.list_chunks()?)
    }

    /// Get a raw chunk.
    /// `slot_id` is a StackerDB chunk ID.
    /// The signature of the chunk will *not* be checked; use `fetch_chunk` for that.
    /// Returns true if we got chunk data
    /// Returns false if there is no chunk data
    /// Returns an error on network errors or codec errors.
    /// In particular, NoSuchChunk means that the node reported that this chunk doesn't exist yet.
    pub fn get_raw_chunk(
        &mut self,
        slot_id: u32,
        data_hash: &Sha512Trunc256Sum,
    ) -> Result<Vec<u8>, Error> {
        let chunks = self.replica_client.get_latest_chunks(&[slot_id])?;
        let Some(chunk_opt) = chunks.get(0) else {
            wrb_debug!("No such StackerDB chunk {}", slot_id);
            return Err(Error::NoSuchChunk);
        };
        let Some(chunk) = chunk_opt else {
            wrb_debug!("No data for StackerDB chunk {}", slot_id);
            return Err(Error::NoSuchChunk);
        };
        if data_hash != &Sha512Trunc256Sum::from_data(&chunk) {
            return Err(Error::GetChunk("chunk hash mismatch".into()));
        }
        Ok(chunk.clone())
    }

    /// Get and authenticate raw chunk.
    /// `slot_id` is a StackerDB chunk ID.
    /// Returns true if we got chunk data
    /// Returns false if there is no chunk data
    /// Returns an error on network errors or codec errors.
    /// In particular, NoSuchChunk means that the node reported that this chunk doesn't exist yet.
    pub fn get_and_verify_raw_chunk(&mut self, slot_id: u32) -> Result<Option<Vec<u8>>, Error> {
        if self.signers.is_none() {
            self.refresh_signers()?;
        }
        let Some(signers) = self.signers.as_ref() else {
            return Err(Error::GetChunk("Unable to load signer list".into()));
        };
        let Some(signer_addr) = signers.get(slot_id as usize).cloned() else {
            return Err(Error::GetChunk(format!(
                "No such signer for chunk ID {}",
                slot_id
            )));
        };
        let all_slot_metadata = self.replica_client.list_chunks()?;
        let slot_md = all_slot_metadata
            .get(slot_id as usize)
            .ok_or(Error::GetChunk(
                "no app chunk defined in slot metadata".into(),
            ))?;
        if slot_md.slot_version == 0 && slot_md.data_hash == Sha512Trunc256Sum([0x00; 32]) {
            // no chunk at all
            return Ok(None);
        }
        if !slot_md.verify(&signer_addr).map_err(|e| {
            Error::GetChunk(format!(
                "Failed to verify signature on {:?}: {:?}",
                &slot_md, &e
            ))
        })? {
            wrb_warn!(
                "Slot not signed by signer; signer_addr = {}, metadata = {:?}",
                &signer_addr,
                &slot_md
            );
            return Err(Error::GetChunk("Invalid chunk signature".into()));
        }

        // hash is authentic
        let chunk = self.get_raw_chunk(slot_id, &slot_md.data_hash)?;
        Ok(Some(chunk))
    }

    /// Get a chunk (a bundle of slices) and cache it locally.
    /// `slot_id` is a StackerDB chunk ID.
    /// The signature of the chunk will *not* be checked; use `fetch_chunk` for that.
    /// Returns true if we got chunk data
    /// Returns false if there is no chunk data
    /// Returns an error on network errors or codec errors.
    /// In particular, NoSuchChunk means that the node reported that this chunk doesn't exist yet.
    pub fn get_chunk(&mut self, slot_id: u32, data_hash: &Sha512Trunc256Sum) -> Result<(), Error> {
        let chunk = self.get_raw_chunk(slot_id, data_hash)?;
        let slices = WrbpodSlices::from_slice(&chunk)?;
        self.chunks.insert(slot_id, slices);
        Ok(())
    }

    /// Get a reference to a downloaded chunk
    /// The slot_id is a StackerDB slot ID
    pub fn ref_chunk(&self, slot_id: u32) -> Option<&WrbpodSlices> {
        self.chunks.get(&slot_id)
    }

    /// Get a reference to a downloaded chunk, addressed by app
    pub fn ref_app_chunk(&self, app_name: &str, app_slot_id: u32) -> Option<&WrbpodSlices> {
        let Some(slot_id) = self.app_slot_id_to_stackerdb_chunk_id(app_name, app_slot_id) else {
            return None;
        };
        self.chunks.get(&slot_id)
    }

    /// Get the digest to sign that authenticates this chunk data and metadata
    fn chunk_auth_digest(
        slot_id: u32,
        slot_version: u32,
        data_hash: &Sha512Trunc256Sum,
    ) -> Sha512Trunc256Sum {
        let mut data = vec![];
        data.extend_from_slice(&slot_id.to_be_bytes());
        data.extend_from_slice(&slot_version.to_be_bytes());
        data.extend_from_slice(&data_hash.0);
        Sha512Trunc256Sum::from_data(&data)
    }

    /// Convert an app slot ID to a stackerdb slot ID
    pub fn app_slot_id_to_stackerdb_chunk_id(
        &self,
        app_name: &str,
        app_slot_id: u32,
    ) -> Option<u32> {
        let Some(chunk_id) = self
            .superblock
            .app_slot_id_to_stackerdb_chunk_id(app_name, app_slot_id)
        else {
            return None;
        };
        Some(chunk_id)
    }

    /// Fetch a chunk for an app, and cache it locally.
    /// Returns the chunk version and signer public key.
    pub fn fetch_chunk(
        &mut self,
        app_name: &str,
        app_slot_id: u32,
    ) -> Result<(u32, Option<StacksPublicKey>), Error> {
        let mut refreshed_signers = false;
        loop {
            let Some(chunk_id) = self
                .superblock
                .app_slot_id_to_stackerdb_chunk_id(app_name, app_slot_id)
            else {
                return Err(Error::GetChunk("no such app chunk".into()));
            };
            let all_slot_metadata = self.replica_client.list_chunks()?;
            let slot_md = all_slot_metadata
                .get(chunk_id as usize)
                .ok_or(Error::GetChunk(
                    "no app chunk defined in slot metadata".into(),
                ))?;

            if self.signers.is_none() {
                self.refresh_signers()?;
                refreshed_signers = true;
            }
            let Some(signers) = self.signers.as_ref() else {
                return Err(Error::GetChunk("Unable to load signer list".into()));
            };
            let Some(signer_addr) = signers.get(chunk_id as usize).cloned() else {
                return Err(Error::GetChunk(format!(
                    "No such signer for chunk ID {}",
                    chunk_id
                )));
            };

            if slot_md.slot_version == 0 && slot_md.data_hash == Sha512Trunc256Sum([0x00; 32]) {
                // this slot is empty, so pass a null signer
                return Ok((0, None));
            }

            if !slot_md.verify(&signer_addr).map_err(|e| {
                Error::GetChunk(format!(
                    "Failed to verify signature on {:?}: {:?}",
                    &slot_md, &e
                ))
            })? {
                if !refreshed_signers {
                    // try again after refreshing the signers
                    self.refresh_signers()?;
                    refreshed_signers = true;
                    continue;
                }

                // already refreshed signers
                wrb_warn!(
                    "Slot not signed by signer; signer_addr = {}, metadata = {:?}",
                    &signer_addr,
                    &slot_md
                );
                return Err(Error::GetChunk("Invalid chunk signature".into()));
            }

            self.get_chunk(chunk_id, &slot_md.data_hash)?;

            let sigh = Self::chunk_auth_digest(chunk_id, slot_md.slot_version, &slot_md.data_hash);
            let pubk = StacksPublicKey::recover_to_pubkey(sigh.as_bytes(), &slot_md.signature)
                .map_err(|_| Error::GetChunk("failed to recover public key".into()))?;

            return Ok((slot_md.slot_version, Some(pubk)));
        }
    }

    /// Get a slice, given the StackerDB chunk ID and the slice ID
    pub fn get_slice(&self, app_name: &str, app_slot_id: u32, id: u128) -> Option<Vec<u8>> {
        let Some(chunk_id) = self
            .superblock
            .app_slot_id_to_stackerdb_chunk_id(app_name, app_slot_id)
        else {
            return None;
        };
        let Some(slices) = self.chunks.get(&chunk_id) else {
            return None;
        };
        slices.get_slice(id).cloned()
    }

    /// Can a slice of a given length be stored?
    pub fn can_fit_slice(
        &self,
        app_name: &str,
        app_slot_id: u32,
        slice_id: u128,
        size: usize,
    ) -> bool {
        let Some(chunk_id) = self
            .superblock
            .app_slot_id_to_stackerdb_chunk_id(app_name, app_slot_id)
        else {
            return false;
        };
        if let Some(slot_slices) = self.chunks.get(&chunk_id) {
            slot_slices.can_fit_slice(slice_id, size)
        } else {
            WrbpodSlices::slice_encoded_size(size, false) + WRBPOD_SLICES_INITIAL_SIZE
                < WRBPOD_CHUNK_MAX_SIZE.into()
        }
    }

    /// Add a slice to our local copy of the slices.
    /// Return true if inserted
    /// Return false if this chunk isn't mapped to our app or if it would make the chunk too big
    pub fn put_slice(
        &mut self,
        app_name: &str,
        app_slot_id: u32,
        slice_id: u128,
        slice_bytes: Vec<u8>,
    ) -> bool {
        let Some(chunk_id) = self
            .superblock
            .app_slot_id_to_stackerdb_chunk_id(app_name, app_slot_id)
        else {
            return false;
        };
        if let Some(slot_slices) = self.chunks.get_mut(&chunk_id) {
            return slot_slices.put_slice(slice_id, slice_bytes);
        } else {
            let mut slices = WrbpodSlices::new();
            if !slices.put_slice(slice_id, slice_bytes) {
                return false;
            }
            self.chunks.insert(chunk_id, slices);
            return true;
        }
    }

    /// Save a dirty slot
    pub fn sync_slot(&mut self, app_name: &str, app_slot_id: u32) -> Result<(), Error> {
        let Some(chunk_id) = self
            .superblock
            .app_slot_id_to_stackerdb_chunk_id(app_name, app_slot_id)
        else {
            return Err(Error::NoSuchChunk);
        };
        let Some(slices) = self.chunks.get_mut(&chunk_id) else {
            return Err(Error::NoSuchChunk);
        };

        let slot_metadata = self.replica_client.list_chunks()?;
        let chunk_id_usize = usize::try_from(chunk_id)
            .map_err(|_| Error::Overflow("could not convert slot ID to usize".into()))?;

        let Some(slot_md) = slot_metadata.get(chunk_id_usize) else {
            wrb_warn!(
                "Could not save chunk {}: no longer in slot metadata",
                &chunk_id
            );
            return Err(Error::NoSuchChunk);
        };
        let chunk = slices.to_stackerdb_chunk(chunk_id, slot_md.slot_version + 1);

        self.put_chunk(chunk)?;

        let Some(slices) = self.chunks.get_mut(&chunk_id) else {
            return Err(Error::NoSuchChunk);
        };
        slices.set_dirty(false);
        Ok(())
    }
}
