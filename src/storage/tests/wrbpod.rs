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

use crate::runner::Error as RuntimeError;
use crate::storage::StackerDBClient;
use crate::storage::WrbpodSlices;
use crate::storage::WRBPOD_SLICES_VERSION;

use crate::ui::Renderer;

use crate::vm::ClarityVM;

use stacks_common::types::chainstate::StacksPrivateKey;
use stacks_common::util::hash::Sha512Trunc256Sum;

use libstackerdb::{SlotMetadata, StackerDBChunkAckData, StackerDBChunkData};

use crate::core;
use crate::core::Config;
use crate::runner::Runner;

#[test]
fn test_wrbpod_open() {
    core::init(true, "localhost", 20443);

    let db_path = "/tmp/wrb-wrbpod-open";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let code = r#"
    (wrb-root u80 u1)
    (wrb-viewport u0 u0 u0 u80 u1)

    ;; open once
    (let (
        (wrbpod-session-id (unwrap-panic (wrbpod-open 'SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod)))
    )
        (asserts! (is-eq wrbpod-session-id u1) (err "Did not open"))
    )
    
    ;; open again
    (let (
        (wrbpod-session-id (unwrap-panic (wrbpod-open 'SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod)))
    )
        (asserts! (is-eq wrbpod-session-id u1) (err "Did not open"))
    )

    ;; open a different one
    (let (
        (wrbpod-session-id (unwrap-panic (wrbpod-open 'SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod-2)))
    )
        (asserts! (is-eq wrbpod-session-id u2) (err "Did not open"))
    )
    "#;

    let bytes = Renderer::encode_bytes(code.as_bytes()).unwrap();

    let mut vm = ClarityVM::new(db_path, "foo.btc").unwrap();
    let mut renderer = Renderer::new(1_000_000_000);
    let s = renderer.eval_to_text(&mut vm, &bytes).unwrap();
    println!("text '{}'", &s);
}

#[test]
fn test_wrbpod_slots() {
    core::init(true, "localhost", 20443);

    let db_path = "/tmp/wrb-wrbpod-slots";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let code = r#"
    (wrb-root u80 u1)
    (wrb-viewport u0 u0 u0 u80 u1)

    ;; open 
    (let (
        (wrbpod-session-id (unwrap-panic (wrbpod-open 'SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod)))
        (num-slots (unwrap-panic (wrbpod-get-num-slots wrbpod-session-id { name: 0x666f6f, namespace: 0x627463 })))
    )
        (asserts! (is-eq wrbpod-session-id u1) (err "Did not open"))
        (asserts! (is-eq num-slots u0) (err "Had slots for unalloced wrbpod"))
        (asserts! (is-err (wrbpod-fetch-slot wrbpod-session-id u0)) (err "fetched nonexistent slot"))
    )
   
    ;; allocate slots
    (let (
        (wrbpod-session-id (unwrap-panic (wrbpod-open 'SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod)))
        (wrbpod-alloc-success (unwrap-panic (wrbpod-alloc-slots wrbpod-session-id u1)))
    ) 
        (asserts! wrbpod-alloc-success (err "Successful allocation failed"))
    )

    ;; check allocation
    (let (
        (wrbpod-session-id (unwrap-panic (wrbpod-open 'SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod)))
        (num-slots (unwrap-panic (wrbpod-get-num-slots wrbpod-session-id { name: 0x666f6f, namespace: 0x627463 })))
        (slot-md (unwrap-panic (wrbpod-fetch-slot wrbpod-session-id u0)))
    )
        (asserts! (is-eq wrbpod-session-id u1) (err { msg: "Did not open", val: wrbpod-session-id }))
        (asserts! (is-eq num-slots u1) (err { msg: "Wrong number of slots", val: num-slots }))
        (asserts! (is-eq slot-md { version: u0, signer: none }) (err { msg: "wrong md", val: (get version slot-md) }))
    )

    (let (
        (wrbpod-session-id (unwrap-panic (wrbpod-open 'SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod)))
    )
        ;; check failures
        (asserts! (is-err (wrbpod-alloc-slots (+ u1 wrbpod-session-id) u1)) (err "alloc'ed slots for a nonexistant wrbpod"))
        (asserts! (is-err (wrbpod-alloc-slots wrbpod-session-id u4294967297)) (err "alloc'ed too many slots"))
        (asserts! (is-err (wrbpod-fetch-slot wrbpod-session-id u1)) (err "fetched nonexistent slot"))
    )
    "#;

    let bytes = Renderer::encode_bytes(code.as_bytes()).unwrap();

    let mut vm = ClarityVM::new(db_path, "foo.btc").unwrap();
    let mut renderer = Renderer::new(1_000_000_000);
    let s = renderer.eval_to_text(&mut vm, &bytes).unwrap();
    println!("text '{}'", &s);
}

#[test]
fn test_wrbpod_dirty_slices() {
    core::init(true, "localhost", 20443);

    let db_path = "/tmp/wrb-wrbpod-dirty-slices";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let code = r#"
    (wrb-root u80 u1)
    (wrb-viewport u0 u0 u0 u80 u1)

    ;; open and allocate 
    (let (
        (wrbpod-session-id (unwrap-panic (wrbpod-open 'SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod)))
        (wrbpod-alloc-success (unwrap-panic (wrbpod-alloc-slots wrbpod-session-id u1)))
        (num-slots (unwrap-panic (wrbpod-get-num-slots wrbpod-session-id { name: 0x666f6f, namespace: 0x627463 })))
    )
        (asserts! (is-eq wrbpod-session-id u1) (err "Did not open"))
        (asserts! wrbpod-alloc-success (err "Successful allocation failed"))
        (asserts! (is-eq num-slots u1) (err "Allocation failed"))
    )
   
    ;; put a slice
    (let (
        (wrbpod-session-id (unwrap-panic (wrbpod-open 'SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod)))
        (fetchslot-res (wrbpod-fetch-slot wrbpod-session-id u0))
        (putslice-res (wrbpod-put-slice wrbpod-session-id u0 u0 0x001122334455))
    )
        (asserts! (is-ok fetchslot-res) (err "failed to fetch slot"))
        (asserts! (is-ok putslice-res) (err "failed to put slice"))
    )

    ;; can get the slice back since it's dirty
    (let (
        (wrbpod-session-id (unwrap-panic (wrbpod-open 'SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod)))
        (getslice-res (wrbpod-get-slice wrbpod-session-id u0 u0))
    )
        (asserts! (is-ok getslice-res) (err "failed to load dirty slice"))
        (asserts! (is-eq getslice-res (ok 0x001122334455)) (err "got back wrong slice"))
    )
    "#;

    let bytes = Renderer::encode_bytes(code.as_bytes()).unwrap();

    let mut vm = ClarityVM::new(db_path, "foo.btc").unwrap();
    let mut renderer = Renderer::new(1_000_000_000);
    let s = renderer.eval_to_text(&mut vm, &bytes).unwrap();
    println!("text '{}'", &s);
}

#[test]
fn test_wrbpod_sync_slot() {
    core::init(true, "localhost", 20443);

    let db_path = "/tmp/wrb-wrbpod-sync-slot";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let code = r#"
    (wrb-root u80 u1)
    (wrb-viewport u0 u0 u0 u80 u1)

    ;; open and allocate 
    (let (
        (wrbpod-session-id (unwrap-panic (wrbpod-open 'SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod)))
        (wrbpod-alloc-success (unwrap-panic (wrbpod-alloc-slots wrbpod-session-id u1)))
        (num-slots (unwrap-panic (wrbpod-get-num-slots wrbpod-session-id { name: 0x666f6f, namespace: 0x627463 })))
    )
        (asserts! (is-eq wrbpod-session-id u1) (err "Did not open"))
        (asserts! wrbpod-alloc-success (err "Successful allocation failed"))
        (asserts! (is-eq num-slots u1) (err "Allocation failed"))
    )
   
    ;; put a slice
    (let (
        (wrbpod-session-id (unwrap-panic (wrbpod-open 'SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod)))
        (fetchslot-res (wrbpod-fetch-slot wrbpod-session-id u0))
        (putslice-res (wrbpod-put-slice wrbpod-session-id u0 u0 0x001122334455))
        (getslice-res (wrbpod-get-slice wrbpod-session-id u0 u0))
    )
        (asserts! (is-ok fetchslot-res) (err "failed to fetch slot"))
        (asserts! (is-ok putslice-res) (err "failed to put slice"))
        (asserts! (is-eq getslice-res (ok 0x001122334455)) (err "got back wrong slice"))
    )

    ;; store the modified slot
    (let (
        (wrbpod-session-id (unwrap-panic (wrbpod-open 'SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod)))
        (sync-res (wrbpod-sync-slot wrbpod-session-id u0))
    )
        (asserts! (is-ok sync-res) (err "failed to sync"))
    )

    ;; idempotent
    (let (
        (wrbpod-session-id (unwrap-panic (wrbpod-open 'SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod)))
        (sync-res (wrbpod-sync-slot wrbpod-session-id u0))
    )
        (asserts! (is-ok sync-res) (err "failed to sync"))
    )

    ;; errors
    (let (
        (wrbpod-session-id (unwrap-panic (wrbpod-open 'SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod)))
    )
        (asserts! (is-err (wrbpod-sync-slot (+ u1 wrbpod-session-id) u0)) (err "synced non-open session"))
        (asserts! (is-err (wrbpod-sync-slot wrbpod-session-id u1)) (err "synced non-open slot"))
    )
    "#;

    let bytes = Renderer::encode_bytes(code.as_bytes()).unwrap();

    let mut vm = ClarityVM::new(db_path, "foo.btc").unwrap();
    let mut renderer = Renderer::new(1_000_000_000);
    let s = renderer.eval_to_text(&mut vm, &bytes).unwrap();
    println!("text '{}'", &s);
}
