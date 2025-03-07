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

use std::fs;
use std::fs::File;
use std::io::Write;

use clarity::vm::types::QualifiedContractIdentifier;
use clarity::vm::Value;

use crate::ui::ValueExtensions;

use crate::cli::clar::json_to_clarity;
use crate::cli::subcommand_wrbpod;
use crate::cli::wrbpod::wrbpod_open_session;
use crate::core;
use crate::core::with_globals;

use crate::stacks_common::codec::StacksMessageCodec;
use stacks_common::util::hash::to_hex;

use crate::storage::WrbpodAddress;
use crate::storage::WrbpodSlices;
use crate::storage::WrbpodSlot;
use crate::storage::WrbpodSuperblock;

#[test]
fn test_wrbpod_format() {
    core::init(true, "localhost", 20443);

    let wrb_src_path = "/tmp/test-wrbpod-format.clar";
    if fs::metadata(&wrb_src_path).is_ok() {
        fs::remove_file(&wrb_src_path).unwrap();
    }
    let mut wrb_src = fs::File::create(&wrb_src_path).unwrap();
    wrb_src.write_all(br#"(print "hello world")"#).unwrap();
    drop(wrb_src);

    let wrbpod_addr = WrbpodAddress::new(
        QualifiedContractIdentifier::parse("SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod")
            .unwrap(),
        3,
    );

    let args = vec![
        "wrb-test".to_string(),
        "wrbpod".to_string(),
        "format".to_string(),
        "-w".to_string(),
        wrbpod_addr.to_string(),
        "hello-formats.btc".to_string(),
        "1".to_string(),
    ];

    subcommand_wrbpod(args, Some(wrb_src_path.to_string()));

    // check superblock
    let superblock_opt = with_globals(|globals| {
        let wrbpod_session = globals.get_wrbpod_session(0)?;
        let superblock = wrbpod_session.superblock();
        Some(superblock.clone())
    });

    let superblock = superblock_opt.unwrap();

    // slot 3 is the superblock
    assert_eq!(
        superblock,
        WrbpodSuperblock::new(vec![0, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15])
    );
}

#[test]
fn test_get_wrbpod_superblock() {
    core::init(true, "localhost", 20443);
    let contract_addr =
        QualifiedContractIdentifier::parse("SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod")
            .unwrap();
    let wrbpod_addr = WrbpodAddress::new(contract_addr, 0);
    wrbpod_open_session(&wrbpod_addr).unwrap();

    let superblock = with_globals(|globals| {
        let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
        wrbpod_session.superblock().clone()
    });

    println!("{}", serde_json::to_string(&superblock).unwrap());
}

#[test]
fn test_wrbpod_alloc_slots() {
    core::init(true, "localhost", 20443);

    let wrb_src_path = "/tmp/test-wrbpod-alloc-slots.clar";
    if fs::metadata(&wrb_src_path).is_ok() {
        fs::remove_file(&wrb_src_path).unwrap();
    }
    let mut wrb_src = fs::File::create(&wrb_src_path).unwrap();
    wrb_src.write_all(br#"(print "hello world")"#).unwrap();
    drop(wrb_src);

    let wrbpod_addr = WrbpodAddress::new(
        QualifiedContractIdentifier::parse("SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod")
            .unwrap(),
        0,
    );

    let args = vec![
        "wrb-test".to_string(),
        "wrbpod".to_string(),
        "alloc-slots".to_string(),
        "-w".to_string(),
        wrbpod_addr.to_string(),
        "hello-alloc-slots.btc".to_string(),
        "1".to_string(),
    ];

    subcommand_wrbpod(args, Some(wrb_src_path.to_string()));

    // check superblock
    let app_state = with_globals(|globals| {
        let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
        let superblock = wrbpod_session.superblock();
        let app_state = superblock.app_state("hello-alloc-slots.btc").unwrap();
        (*app_state).clone()
    });

    assert_eq!(app_state.slots, vec![1]);
}

#[test]
fn test_wrbpod_get_put_slot() {
    core::init(true, "localhost", 20443);

    let wrb_src_path = "/tmp/test-wrbpod-get-put-slot.clar";
    if fs::metadata(&wrb_src_path).is_ok() {
        fs::remove_file(&wrb_src_path).unwrap();
    }
    let mut wrb_src = fs::File::create(&wrb_src_path).unwrap();
    wrb_src.write_all(br#"(print "hello world")"#).unwrap();
    drop(wrb_src);

    // need to alloc slots first
    let wrbpod_addr = WrbpodAddress::new(
        QualifiedContractIdentifier::parse("SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod")
            .unwrap(),
        0,
    );
    let args = vec![
        "wrb-test".to_string(),
        "wrbpod".to_string(),
        "alloc-slots".to_string(),
        "-w".to_string(),
        wrbpod_addr.to_string(),
        "hello-get-put-slot.btc".to_string(),
        "1".to_string(),
    ];

    subcommand_wrbpod(args, Some(wrb_src_path.to_string()));

    // check superblock
    let app_state = with_globals(|globals| {
        let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
        let superblock = wrbpod_session.superblock();
        let app_state = superblock.app_state("hello-get-put-slot.btc").unwrap();
        (*app_state).clone()
    });

    assert_eq!(app_state.slots, vec![1]);

    // make a slice
    let mut slices = WrbpodSlices::new();
    slices.put_slice(0, vec![1, 2, 3, 4, 5]);

    // put the slot
    let args = vec![
        "wrb-test".to_string(),
        "wrbpod".to_string(),
        "put-slot".to_string(),
        "-w".to_string(),
        wrbpod_addr.to_string(),
        "hello-get-put-slot.btc".to_string(),
        "0".to_string(),
        serde_json::to_string(&slices).unwrap(),
    ];

    subcommand_wrbpod(args, Some(wrb_src_path.to_string()));

    // go get the slot
    let mut slot = with_globals(|globals| {
        let wrbpod_session = globals.get_wrbpod_session(0).unwrap();
        wrbpod_session
            .fetch_chunk("hello-get-put-slot.btc", 0)
            .map_err(|e| {
                panic!("FATAL: failed to fetch chunk: {:?}", &e);
            })
            .unwrap();

        let chunk_ref = wrbpod_session.ref_chunk(1).unwrap();

        (*chunk_ref).clone()
    });

    slot.set_dirty(true);
    assert_eq!(slot, slices);
}

#[test]
fn test_mulitenant_wrbpod_alloc() {
    core::init(true, "localhost", 20443);

    let wrb_src_path = "/tmp/test-multitenant-wrbpod-alloc-slots.clar";
    if fs::metadata(&wrb_src_path).is_ok() {
        fs::remove_file(&wrb_src_path).unwrap();
    }
    let mut wrb_src = fs::File::create(&wrb_src_path).unwrap();
    wrb_src.write_all(br#"(print "hello world")"#).unwrap();
    drop(wrb_src);

    // wrbpod 1, superblock at slot 0
    let wrbpod_addr_1 = WrbpodAddress::new(
        QualifiedContractIdentifier::parse("SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod")
            .unwrap(),
        0,
    );

    // wrbpod 2, superblock at slot 3
    let wrbpod_addr_2 = WrbpodAddress::new(
        QualifiedContractIdentifier::parse("SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod")
            .unwrap(),
        3,
    );

    // wrbpod 3, superblock at slot 6
    let wrbpod_addr_3 = WrbpodAddress::new(
        QualifiedContractIdentifier::parse("SP1B62RVBBP8N4K3X4K6AA8FFPXQWGGX48SSEKPAB.wrbpod")
            .unwrap(),
        6,
    );

    let expected_app_slots = vec![
        vec![1, 2, 3],
        vec![0, 1, 2, 4, 5, 6],
        vec![0, 1, 2, 3, 4, 5, 7, 8, 9],
    ];

    let expected_available_slots = vec![
        vec![
            WrbpodSlot::Filled(1),
            WrbpodSlot::Filled(2),
            WrbpodSlot::Filled(3),
            WrbpodSlot::Free(4),
            WrbpodSlot::Free(5),
            WrbpodSlot::Free(6),
            WrbpodSlot::Free(7),
            WrbpodSlot::Free(8),
            WrbpodSlot::Free(9),
            WrbpodSlot::Free(10),
            WrbpodSlot::Free(11),
            WrbpodSlot::Free(12),
            WrbpodSlot::Free(13),
            WrbpodSlot::Free(14),
            WrbpodSlot::Free(15),
        ],
        vec![
            WrbpodSlot::Filled(0),
            WrbpodSlot::Filled(1),
            WrbpodSlot::Filled(2),
            WrbpodSlot::Filled(4),
            WrbpodSlot::Filled(5),
            WrbpodSlot::Filled(6),
            WrbpodSlot::Free(7),
            WrbpodSlot::Free(8),
            WrbpodSlot::Free(9),
            WrbpodSlot::Free(10),
            WrbpodSlot::Free(11),
            WrbpodSlot::Free(12),
            WrbpodSlot::Free(13),
            WrbpodSlot::Free(14),
            WrbpodSlot::Free(15),
        ],
        vec![
            WrbpodSlot::Filled(0),
            WrbpodSlot::Filled(1),
            WrbpodSlot::Filled(2),
            WrbpodSlot::Filled(3),
            WrbpodSlot::Filled(4),
            WrbpodSlot::Filled(5),
            WrbpodSlot::Filled(7),
            WrbpodSlot::Filled(8),
            WrbpodSlot::Filled(9),
            WrbpodSlot::Free(10),
            WrbpodSlot::Free(11),
            WrbpodSlot::Free(12),
            WrbpodSlot::Free(13),
            WrbpodSlot::Free(14),
            WrbpodSlot::Free(15),
        ],
    ];

    for (i, wrbpod_addr) in [&wrbpod_addr_1, &wrbpod_addr_2, &wrbpod_addr_3]
        .iter()
        .enumerate()
    {
        let args = vec![
            "wrb-test".to_string(),
            "wrbpod".to_string(),
            "alloc-slots".to_string(),
            "-w".to_string(),
            wrbpod_addr.to_string(),
            format!("hello-alloc-slots-{}.btc", i),
            format!("{}", 3 * (i + 1)),
        ];

        subcommand_wrbpod(args, Some(wrb_src_path.to_string()));
    }

    for (i, wrbpod_addr) in [&wrbpod_addr_1, &wrbpod_addr_2, &wrbpod_addr_3]
        .iter()
        .enumerate()
    {
        // check superblock
        let superblock = with_globals(|globals| {
            let wrbpod_session = globals.get_wrbpod_session_by_address(wrbpod_addr).unwrap();
            let superblock = wrbpod_session.superblock();
            superblock.clone()
        });

        eprintln!("{:?}", &superblock);

        assert_eq!(superblock.slot_ids.len(), 15); // default has 160
        assert_eq!(superblock.slot_ids, expected_available_slots[i]);

        let app_name = format!("hello-alloc-slots-{}.btc", i);
        assert_eq!(
            superblock.apps.get(&app_name).unwrap().slots,
            expected_app_slots[i]
        );
    }
}
