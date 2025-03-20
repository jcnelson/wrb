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

use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;

use crate::runner::Error as RuntimeError;
use crate::storage::StackerDBClient;
use crate::storage::WrbpodSlices;
use crate::storage::WRBPOD_SLICES_VERSION;

use crate::ui::Renderer;

use crate::vm::ClarityVM;

use stacks_common::types::chainstate::StacksAddress;
use stacks_common::types::chainstate::StacksPrivateKey;
use stacks_common::types::chainstate::StacksPublicKey;
use stacks_common::util::hash::Sha512Trunc256Sum;

use libstackerdb::{SlotMetadata, StackerDBChunkAckData, StackerDBChunkData};

use crate::core;
use crate::core::Config;
use crate::runner::Runner;

mod mock;
mod wrbpod;

#[derive(Clone)]
pub struct MockStackerDBClient {
    pub privkey: StacksPrivateKey,
    pub num_slots: u32,
    pub mock_failure: Option<RuntimeError>,
    pub mock_ack_failure: Option<StackerDBChunkAckData>,
    pub chunks: HashMap<u32, StackerDBChunkData>,
}

impl MockStackerDBClient {
    pub fn new(privk: StacksPrivateKey, num_slots: u32) -> Self {
        Self {
            privkey: privk,
            num_slots,
            mock_failure: None,
            mock_ack_failure: None,
            chunks: HashMap::new(),
        }
    }
}

impl StackerDBClient for MockStackerDBClient {
    fn get_host(&self) -> SocketAddr {
        let addr: SocketAddr = "127.0.0.1:30443".parse().unwrap();
        addr
    }

    fn list_chunks(&mut self) -> Result<Vec<SlotMetadata>, RuntimeError> {
        if let Some(error) = self.mock_failure.as_ref() {
            return Err(error.clone());
        }

        let mut metadata = vec![];
        for slot_id in 0..self.num_slots {
            if let Some(chunk) = self.chunks.get(&slot_id) {
                metadata.push(chunk.get_slot_metadata());
            } else {
                metadata.push(SlotMetadata::new_unsigned(
                    slot_id,
                    0,
                    Sha512Trunc256Sum([0x00; 32]),
                ));
            }
        }
        wrb_test_debug!("list_chunks: {:?}", &metadata);
        Ok(metadata)
    }

    fn get_chunks(
        &mut self,
        slots_and_versions: &[(u32, u32)],
    ) -> Result<Vec<Option<Vec<u8>>>, RuntimeError> {
        if let Some(error) = self.mock_failure.as_ref() {
            return Err(error.clone());
        }

        let mut ret = vec![];
        for (slot_id, version) in slots_and_versions.iter() {
            if let Some(chunk) = self.chunks.get(slot_id) {
                if chunk.slot_version == *version {
                    ret.push(Some(chunk.data.clone()));
                } else {
                    ret.push(None);
                }
            } else {
                ret.push(None);
            }
        }
        wrb_test_debug!("get_chunks({:?}): {:?}", slots_and_versions, &ret);
        Ok(ret)
    }

    fn get_latest_chunks(
        &mut self,
        slot_ids: &[u32],
    ) -> Result<Vec<Option<Vec<u8>>>, RuntimeError> {
        if let Some(error) = self.mock_failure.as_ref() {
            return Err(error.clone());
        }

        let mut ret = vec![];
        for slot_id in slot_ids.iter() {
            if let Some(chunk) = self.chunks.get(slot_id) {
                ret.push(Some(chunk.data.clone()));
            } else {
                ret.push(None);
            }
        }
        wrb_test_debug!("get_latest_chunks({:?}): {:?}", slot_ids, &ret);
        Ok(ret)
    }

    fn put_chunk(
        &mut self,
        chunk: StackerDBChunkData,
    ) -> Result<StackerDBChunkAckData, RuntimeError> {
        if let Some(error) = self.mock_failure.as_ref() {
            return Err(error.clone());
        }
        if let Some(bad_ack) = self.mock_ack_failure.as_ref() {
            return Ok(bad_ack.clone());
        }
        self.chunks.insert(chunk.slot_id, chunk.clone());

        let ret = StackerDBChunkAckData {
            accepted: true,
            reason: None,
            metadata: None,
            code: None,
        };
        wrb_test_debug!("put_chunk({:?}): {:?}", &chunk, &ret);
        Ok(ret)
    }

    fn find_replicas(&mut self) -> Result<Vec<SocketAddr>, RuntimeError> {
        if let Some(error) = self.mock_failure.as_ref() {
            return Err(error.clone());
        }
        return Ok(vec![SocketAddr::from(([127, 0, 0, 1], 20443))]);
    }

    fn get_signers(&mut self) -> Result<Vec<StacksAddress>, RuntimeError> {
        let mut pubkey = StacksPublicKey::from_private(&self.privkey);
        pubkey.set_compressed(true);
        return Ok(vec![StacksAddress::p2pkh(true, &pubkey); 16]);
    }
}

#[test]
fn test_mock_stackerdb() {
    let mut mock_stackerdb = MockStackerDBClient::new(StacksPrivateKey::random(), 3);
    assert_eq!(
        mock_stackerdb.list_chunks().unwrap(),
        vec![
            SlotMetadata::new_unsigned(0, 0, Sha512Trunc256Sum([0x00; 32])),
            SlotMetadata::new_unsigned(1, 0, Sha512Trunc256Sum([0x00; 32])),
            SlotMetadata::new_unsigned(2, 0, Sha512Trunc256Sum([0x00; 32])),
        ]
    );
    assert_eq!(
        mock_stackerdb
            .get_chunks(&[(0, 0), (1, 0), (2, 0)])
            .unwrap(),
        vec![None, None, None]
    );
    assert_eq!(
        mock_stackerdb.get_latest_chunks(&[0, 1, 2]).unwrap(),
        vec![None, None, None]
    );

    let privk = StacksPrivateKey::random();
    let mut chunk = StackerDBChunkData::new(0, 0, vec![1, 2, 3, 4, 5]);
    chunk.sign(&privk).unwrap();

    assert_eq!(
        mock_stackerdb.put_chunk(chunk.clone()).unwrap(),
        StackerDBChunkAckData {
            accepted: true,
            reason: None,
            metadata: None,
            code: None
        }
    );

    assert_eq!(
        mock_stackerdb.list_chunks().unwrap(),
        vec![
            chunk.get_slot_metadata(),
            SlotMetadata::new_unsigned(1, 0, Sha512Trunc256Sum([0x00; 32])),
            SlotMetadata::new_unsigned(2, 0, Sha512Trunc256Sum([0x00; 32])),
        ]
    );
    assert_eq!(
        mock_stackerdb
            .get_chunks(&[(0, 0), (1, 0), (2, 0)])
            .unwrap(),
        vec![Some(chunk.data.clone()), None, None]
    );
    assert_eq!(
        mock_stackerdb
            .get_chunks(&[(0, 1), (1, 0), (2, 0)])
            .unwrap(),
        vec![None, None, None]
    );
    assert_eq!(
        mock_stackerdb.get_latest_chunks(&[0, 1, 2]).unwrap(),
        vec![Some(chunk.data.clone()), None, None]
    );
}

#[test]
fn test_wrbpod_slices() {
    let mut slices = WrbpodSlices::new();

    assert!(slices.get_slice(10).is_none());
    assert!(slices.get_slice(11).is_none());
    assert!(slices.get_slice(12).is_none());
    assert!(!slices.is_dirty());

    slices.put_slice(10, "hello slice 0".as_bytes().to_vec());

    assert_eq!(
        slices.get_slice(10).to_owned().unwrap(),
        &"hello slice 0".as_bytes().to_vec()
    );
    assert!(slices.is_dirty());

    slices.put_slice(11, "hello slice 1".as_bytes().to_vec());
    slices.put_slice(12, "hello slice 2".as_bytes().to_vec());

    assert_eq!(
        slices.get_slice(11).to_owned().unwrap(),
        &"hello slice 1".as_bytes().to_vec()
    );
    assert_eq!(
        slices.get_slice(12).to_owned().unwrap(),
        &"hello slice 2".as_bytes().to_vec()
    );

    let chunk = slices.to_stackerdb_chunk(1, 2);
    assert_eq!(
        chunk.data,
        vec![
            // version
            WRBPOD_SLICES_VERSION,
            // number of slices
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x03,
            // slice ID 10
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x0a,
            // slice index 0
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            // slice ID 11
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x0b,
            // slice index 1
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x01,
            // slice ID 12
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x0c,
            // slice index 2
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x02,
            // slice contents for ID 10
            // length
            0x00,
            0x00,
            0x00,
            0x0d,
            // contents
            0x68,
            0x65,
            0x6c,
            0x6c,
            0x6f,
            0x20,
            0x73,
            0x6c,
            0x69,
            0x63,
            0x65,
            0x20,
            0x30,
            // slice contents for ID 11
            // length
            0x00,
            0x00,
            0x00,
            0x0d,
            // contents
            0x68,
            0x65,
            0x6c,
            0x6c,
            0x6f,
            0x20,
            0x73,
            0x6c,
            0x69,
            0x63,
            0x65,
            0x20,
            0x31,
            // slice contents for ID 12
            // length
            0x00,
            0x00,
            0x00,
            0x0d,
            // contents
            0x68,
            0x65,
            0x6c,
            0x6c,
            0x6f,
            0x20,
            0x73,
            0x6c,
            0x69,
            0x63,
            0x65,
            0x20,
            0x32,
        ]
    );

    let mut parsed_slices = WrbpodSlices::from_slice(&chunk.data).unwrap();
    parsed_slices.set_dirty(true);

    assert_eq!(parsed_slices, slices);
}

#[test]
fn test_wrbpod_slices_serde() {
    let mut slices = WrbpodSlices::new();
    slices.put_slice(10, "hello slice 0".as_bytes().to_vec());
    slices.put_slice(11, "hello slice 1".as_bytes().to_vec());
    slices.put_slice(12, "hello slice 2".as_bytes().to_vec());

    let slices_json = serde_json::to_string(&slices).unwrap();
    eprintln!("{:?}", slices_json);

    let serde_slices: WrbpodSlices = serde_json::from_str(&slices_json).unwrap();
    assert_eq!(serde_slices, slices);
}
