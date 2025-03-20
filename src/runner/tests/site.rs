// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2022 Stacks Open Internet Foundation
// Copyright (C) 2022-2025 Jude Nelson
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

use std::net::SocketAddr;

use crate::runner::site::{WrbTxtRecord, WrbTxtRecordV1, ZonefileResourceRecord};
use crate::runner::tests::BNSNameRecord;
use crate::runner::Error;
use crate::runner::Runner;

use libstackerdb::SlotMetadata;
use libstackerdb::StackerDBChunkData;

use clarity::vm::types::QualifiedContractIdentifier;

use crate::ui::render::Renderer;

use crate::runner::tests::MockBNSResolver;
use crate::storage;
use crate::storage::tests::MockStackerDBClient;
use crate::storage::StackerDBClient;

use base64ct::{Base64, Encoding};

use crate::stacks_common::codec::StacksMessageCodec;

use stacks_common::types::chainstate::StacksAddress;
use stacks_common::types::chainstate::StacksPrivateKey;
use stacks_common::types::chainstate::StacksPublicKey;
use stacks_common::util::hash::Sha512Trunc256Sum;
use stacks_common::util::secp256k1::MessageSignature;

#[test]
fn test_dns_rr_parse() {
    let expected_rrs = vec![
        ZonefileResourceRecord {
            rr_name: "bar".into(),
            rr_ttl: Some(3600),
            rr_class: "IN".into(),
            rr_type: "A".into(),
            rr_payload: "1.2.3.4".into(),
        },
        ZonefileResourceRecord {
            rr_name: "baz".into(),
            rr_ttl: None,
            rr_class: "IN".into(),
            rr_type: "A".into(),
            rr_payload: "5.6.7.8".into(),
        },
        ZonefileResourceRecord {
            rr_name: "_http._tcp".into(),
            rr_ttl: Some(3600),
            rr_class: "IN".into(),
            rr_type: "TXT".into(),
            rr_payload: "\"http://example.com\"".into(),
        },
        ZonefileResourceRecord {
            rr_name: "wrb".into(),
            rr_ttl: None,
            rr_class: "IN".into(),
            rr_type: "TXT".into(),
            rr_payload: "\"asdffdsa\"".into(),
        },
        ZonefileResourceRecord {
            rr_name: "wrb".into(),
            rr_ttl: None,
            rr_class: "IN".into(),
            rr_type: "TXT".into(),
            rr_payload: "\"asdffdsa\",\"quux\"".into(),
        },
    ];

    let zonefile = b"$ORIGIN foo.com\nbar 3600 IN A 1.2.3.4\nbaz IN A 5.6.7.8\n_http._tcp 3600 IN TXT \"\\\"http://example.com\\\"\"\nwrb IN TXT \"\\\"asdffdsa\\\"\"\nwrb IN TXT \"\\\"asdffdsa\\\",\\\"quux\\\"\"".to_vec();
    let rrs = Runner::decode_zonefile_records(zonefile).unwrap();
    assert_eq!(rrs, expected_rrs);

    let zonefile = b"$ORIGIN foo.com\n\nbar \t3600 IN\tA 1.2.3.4\nbaz\t\t\tIN        A \t \t 5.6.7.8\n_http._tcp \t 3600     IN \t\t\t  \t TXT \"\\\"http://example.com\\\"\"\nwrb \t\t\t IN\t \tTXT\t \"\\\"asdffdsa\\\"\"\nwrb IN TXT \"\\\"asdffdsa\\\",\\\"quux\\\"\"\n\n\n    \n".to_vec();
    let rrs = Runner::decode_zonefile_records(zonefile).unwrap();
    assert_eq!(rrs, expected_rrs);

    let zonefile = b"$ORIGIN foo.com\nbad1 IN\nbar 3600 IN A 1.2.3.4\nbad2 IN A\nbaz IN A 5.6.7.8\nbad3 3600\n_http._tcp 3600 IN TXT \"\\\"http://example.com\\\"\"\nbad4 \"bad4\"\nwrb IN TXT \"\\\"asdffdsa\\\"\"\nwrb IN TXT \"\\\"asdffdsa\\\",\\\"quux\\\"\"".to_vec();
    let rrs = Runner::decode_zonefile_records(zonefile).unwrap();
    assert_eq!(rrs, expected_rrs);
}

#[test]
fn test_dns_rr_codec() {
    let payload = "\"1.2\"3.\\4\"";
    assert_eq!(
        ZonefileResourceRecord::escape_string(payload)
            .unwrap()
            .as_str(),
        "\"\\\"1.2\\\"3.\\\\4\\\"\""
    );
    assert_eq!(
        ZonefileResourceRecord::unescape_string(
            &ZonefileResourceRecord::escape_string(payload).unwrap()
        )
        .unwrap()
        .as_str(),
        payload
    );

    let rec = ZonefileResourceRecord {
        rr_name: "bar".into(),
        rr_ttl: Some(3600),
        rr_class: "IN".into(),
        rr_type: "A".into(),
        rr_payload: "\"1.2.\"3.4\"".into(),
    };
    let txt = rec.to_string();
    wrb_debug!("txt = '{}'", &txt);
    let decoded = Runner::decode_zonefile_records(txt.as_bytes().to_vec()).unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0], rec);
}

#[test]
fn test_dns_wrb_txt_codec() {
    let wrbrec = WrbTxtRecord::V1(WrbTxtRecordV1 {
        contract_id: QualifiedContractIdentifier::parse(
            "S1G2081040G2081040G2081040G208105NK8PE5.tokens",
        )
        .unwrap(),
        slot_metadata: SlotMetadata {
            slot_id: 1,
            slot_version: 2,
            data_hash: Sha512Trunc256Sum([0x33; 32]),
            signature: MessageSignature::empty(),
        },
    });

    let wrbrec_bytes = vec![
        // wrbrec version
        0x01, // contract type prefix
        0x06, // address version byte
        0x01, // address bytes
        0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
        0x01, 0x01, 0x01, 0x01, 0x01, // name length
        0x06, // name ('tokens')
        0x74, 0x6f, 0x6b, 0x65, 0x6e, 0x73, // slot_id
        0x00, 0x00, 0x00, 0x01, // slot_version
        0x00, 0x00, 0x00, 0x02, // data_hash
        0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33,
        0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33,
        0x33, 0x33, // signature
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    assert_eq!(wrbrec.serialize_to_vec(), wrbrec_bytes);
    assert_eq!(
        WrbTxtRecord::consensus_deserialize(&mut &wrbrec_bytes[..]).unwrap(),
        wrbrec
    );

    let b64 = Base64::encode_string(&wrbrec_bytes);
    eprintln!("wrbrec txt record ({}): {}", b64.len(), &b64);
    assert!(b64.len() < 256);

    assert_eq!(
        ZonefileResourceRecord::try_from(wrbrec.clone())
            .unwrap()
            .to_string(),
        format!("wrb\t\tIN\tTXT\t\"{}\"", &b64)
    );
}

#[test]
fn test_wrbsite_load_from_zonefile_rec() {
    let pkey = StacksPrivateKey::random();

    let mut pubkey = StacksPublicKey::from_private(&pkey);
    pubkey.set_compressed(true);

    let code_body = b"(print \"hello world!\")";
    let code_bytes = Renderer::encode_bytes(code_body).unwrap();
    let chunk = StackerDBChunkData::new(1, 2, code_bytes.clone());
    let code_hash = chunk.data_hash();

    let mut slot_metadata =
        SlotMetadata::new_unsigned(chunk.slot_id, chunk.slot_version, code_hash.clone());
    slot_metadata.sign(&pkey).unwrap();

    let mut mock_stackerdb = MockStackerDBClient::new(pkey.clone(), 3);
    mock_stackerdb.put_chunk(chunk).unwrap();

    // happy path
    let wrbrec = WrbTxtRecordV1 {
        contract_id: QualifiedContractIdentifier::parse(
            "S1G2081040G2081040G2081040G208105NK8PE5.test",
        )
        .unwrap(),
        slot_metadata,
    };

    let bytes = Runner::wrbsite_load_from_zonefile_rec(&wrbrec, &mut mock_stackerdb)
        .unwrap()
        .unwrap();
    assert_eq!(bytes, code_bytes);

    // sad path -- no such chunk
    let bad_slot_metadata = SlotMetadata::new_unsigned(0, 2, code_hash.clone());
    let wrbrec = WrbTxtRecordV1 {
        contract_id: QualifiedContractIdentifier::parse(
            "S1G2081040G2081040G2081040G208105NK8PE5.test",
        )
        .unwrap(),
        slot_metadata: bad_slot_metadata,
    };

    assert!(matches!(
        Runner::wrbsite_load_from_zonefile_rec(&wrbrec, &mut mock_stackerdb),
        Err(Error::Storage(_))
    ));

    // sad path -- no such version
    let bad_slot_metadata = SlotMetadata::new_unsigned(1, 3, code_hash.clone());
    let wrbrec = WrbTxtRecordV1 {
        contract_id: QualifiedContractIdentifier::parse(
            "S1G2081040G2081040G2081040G208105NK8PE5.test",
        )
        .unwrap(),
        slot_metadata: bad_slot_metadata,
    };

    assert!(matches!(
        Runner::wrbsite_load_from_zonefile_rec(&wrbrec, &mut mock_stackerdb),
        Err(Error::Storage(_))
    ));

    // sad path -- bad hash
    let bad_slot_metadata = SlotMetadata::new_unsigned(1, 2, Sha512Trunc256Sum::from_data(b""));
    let wrbrec = WrbTxtRecordV1 {
        contract_id: QualifiedContractIdentifier::parse(
            "S1G2081040G2081040G2081040G208105NK8PE5.test",
        )
        .unwrap(),
        slot_metadata: bad_slot_metadata,
    };

    assert!(matches!(
        Runner::wrbsite_load_from_zonefile_rec(&wrbrec, &mut mock_stackerdb),
        Err(Error::Storage(_))
    ));
}

fn wrbrec_to_zonefile(wrbrec: WrbTxtRecordV1) -> Vec<u8> {
    let rr_text = ZonefileResourceRecord::try_from(WrbTxtRecord::V1(wrbrec))
        .unwrap()
        .to_string();
    wrb_debug!("rr_text = '{}'", &rr_text);
    format!("$ORIGIN test.test\n\n{}\n\n", &rr_text)
        .as_bytes()
        .to_vec()
}

#[test]
fn test_wrbsite_load_from_zonefile() {
    let pkey = StacksPrivateKey::random();

    let mut pubkey = StacksPublicKey::from_private(&pkey);
    pubkey.set_compressed(true);

    let code_body = b"(print \"hello world!\")";
    let code_bytes = Renderer::encode_bytes(code_body).unwrap();
    let chunk = StackerDBChunkData::new(1, 2, code_bytes.clone());
    let code_hash = chunk.data_hash();

    let mut slot_metadata =
        SlotMetadata::new_unsigned(chunk.slot_id, chunk.slot_version, code_hash.clone());
    slot_metadata.sign(&pkey).unwrap();

    let mut mock_stackerdb = MockStackerDBClient::new(pkey.clone(), 3);
    mock_stackerdb.put_chunk(chunk).unwrap();

    let mut runner = Runner::new(
        QualifiedContractIdentifier::parse("SP2QEZ06AGJ3RKJPBV14SY1V5BBFNAW33D96YPGZF.BNS-V2")
            .unwrap(),
        QualifiedContractIdentifier::parse(
            "SP2QEZ06AGJ3RKJPBV14SY1V5BBFNAW33D96YPGZF.zonefile-resolver",
        )
        .unwrap(),
        "127.0.0.1".to_string(),
        12345,
    );

    // happy path
    let wrbrec = WrbTxtRecordV1 {
        contract_id: QualifiedContractIdentifier::parse(
            "S1G2081040G2081040G2081040G208105NK8PE5.test",
        )
        .unwrap(),
        slot_metadata,
    };

    let (bytes, ver) = runner
        .wrbsite_load_from_zonefile(
            wrbrec_to_zonefile(wrbrec),
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
        )
        .unwrap()
        .unwrap();
    assert_eq!(bytes, code_bytes);
    assert_eq!(ver, 2);

    // sad path -- no such chunk
    let bad_slot_metadata = SlotMetadata::new_unsigned(0, 2, code_hash.clone());
    let wrbrec = WrbTxtRecordV1 {
        contract_id: QualifiedContractIdentifier::parse(
            "S1G2081040G2081040G2081040G208105NK8PE5.test",
        )
        .unwrap(),
        slot_metadata: bad_slot_metadata,
    };

    let err = runner
        .wrbsite_load_from_zonefile(
            wrbrec_to_zonefile(wrbrec),
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
        )
        .unwrap_err();
    assert!(matches!(err, Error::FailedToRun(..)));

    // sad path -- no such chunk
    let bad_slot_metadata = SlotMetadata::new_unsigned(100, 2, code_hash.clone());
    let wrbrec = WrbTxtRecordV1 {
        contract_id: QualifiedContractIdentifier::parse(
            "S1G2081040G2081040G2081040G208105NK8PE5.test",
        )
        .unwrap(),
        slot_metadata: bad_slot_metadata,
    };

    let err = runner
        .wrbsite_load_from_zonefile(
            wrbrec_to_zonefile(wrbrec),
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
        )
        .unwrap_err();
    assert!(matches!(err, Error::FailedToRun(..)));

    // sad path -- no such version
    let bad_slot_metadata = SlotMetadata::new_unsigned(1, 3, code_hash.clone());
    let wrbrec = WrbTxtRecordV1 {
        contract_id: QualifiedContractIdentifier::parse(
            "S1G2081040G2081040G2081040G208105NK8PE5.test",
        )
        .unwrap(),
        slot_metadata: bad_slot_metadata,
    };

    let err = runner
        .wrbsite_load_from_zonefile(
            wrbrec_to_zonefile(wrbrec),
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
        )
        .unwrap_err();
    assert!(matches!(err, Error::FailedToRun(..)));

    // sad path -- bad hash
    let bad_slot_metadata = SlotMetadata::new_unsigned(1, 2, Sha512Trunc256Sum::from_data(b""));
    let wrbrec = WrbTxtRecordV1 {
        contract_id: QualifiedContractIdentifier::parse(
            "S1G2081040G2081040G2081040G208105NK8PE5.test",
        )
        .unwrap(),
        slot_metadata: bad_slot_metadata,
    };

    let err = runner
        .wrbsite_load_from_zonefile(
            wrbrec_to_zonefile(wrbrec),
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
        )
        .unwrap_err();
    assert!(matches!(err, Error::FailedToRun(..)));
}

#[test]
fn test_wrbsite_load_ext() {
    let pkey = StacksPrivateKey::random();

    let mut pubkey = StacksPublicKey::from_private(&pkey);
    pubkey.set_compressed(true);

    let code_body = b"(print \"hello world!\")";
    let code_bytes = Renderer::encode_bytes(code_body).unwrap();
    let chunk = StackerDBChunkData::new(1, 2, code_bytes.clone());
    let code_hash = chunk.data_hash();

    let stackerdb_id =
        QualifiedContractIdentifier::parse("SP2QEZ06AGJ3RKJPBV14SY1V5BBFNAW33D96YPGZF.lolwut")
            .unwrap();

    let mut slot_metadata =
        SlotMetadata::new_unsigned(chunk.slot_id, chunk.slot_version, code_hash.clone());
    slot_metadata.sign(&pkey).unwrap();

    let mut mock_stackerdb = MockStackerDBClient::new(pkey.clone(), 3);
    mock_stackerdb.put_chunk(chunk).unwrap();

    let mut mock_bns_resolver = MockBNSResolver::new();
    mock_bns_resolver.add_name_rec(
        "happy",
        "path",
        BNSNameRecord::from_stackerdb_slot(stackerdb_id.clone(), slot_metadata.clone()),
    );

    // sad path -- empty chunk
    let bad_slot_metadata = SlotMetadata::new_unsigned(0, 2, code_hash.clone());
    mock_bns_resolver.add_name_rec(
        "sad-0",
        "path",
        BNSNameRecord::from_stackerdb_slot(stackerdb_id.clone(), bad_slot_metadata),
    );

    // sad path -- unmapped chunk
    let bad_slot_metadata = SlotMetadata::new_unsigned(100, 2, code_hash.clone());
    mock_bns_resolver.add_name_rec(
        "sad-1",
        "path",
        BNSNameRecord::from_stackerdb_slot(stackerdb_id.clone(), bad_slot_metadata),
    );

    // sad path -- no version
    let bad_slot_metadata = SlotMetadata::new_unsigned(1, 3, code_hash.clone());
    mock_bns_resolver.add_name_rec(
        "sad-2",
        "path",
        BNSNameRecord::from_stackerdb_slot(stackerdb_id.clone(), bad_slot_metadata),
    );

    // sad path -- bad hash
    let bad_slot_metadata = SlotMetadata::new_unsigned(1, 2, Sha512Trunc256Sum::from_data(b""));
    mock_bns_resolver.add_name_rec(
        "sad-3",
        "path",
        BNSNameRecord::from_stackerdb_slot(stackerdb_id.clone(), bad_slot_metadata),
    );

    let mut runner = Runner::new(
        QualifiedContractIdentifier::parse("SP2QEZ06AGJ3RKJPBV14SY1V5BBFNAW33D96YPGZF.BNS-V2")
            .unwrap(),
        QualifiedContractIdentifier::parse(
            "SP2QEZ06AGJ3RKJPBV14SY1V5BBFNAW33D96YPGZF.zonefile-resolver",
        )
        .unwrap(),
        "127.0.0.1".to_string(),
        12345,
    );

    // happy path
    let (resolved_code, ver) = runner
        .wrbsite_load_ext(
            &mut mock_bns_resolver,
            "happy",
            "path",
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
        )
        .unwrap()
        .unwrap();

    assert_eq!(resolved_code, code_bytes);
    assert_eq!(ver, 2);

    // sad path -- empty chunk
    let err = runner
        .wrbsite_load_ext(
            &mut mock_bns_resolver,
            "sad-0",
            "path",
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
        )
        .unwrap_err();
    assert!(matches!(err, Error::FailedToRun(..)));

    // sad path -- unmapped chunk
    let err = runner
        .wrbsite_load_ext(
            &mut mock_bns_resolver,
            "sad-1",
            "path",
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
        )
        .unwrap_err();
    assert!(matches!(err, Error::FailedToRun(..)));

    // sad path -- no version
    let err = runner
        .wrbsite_load_ext(
            &mut mock_bns_resolver,
            "sad-2",
            "path",
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
        )
        .unwrap_err();
    assert!(matches!(err, Error::FailedToRun(..)));

    // sad path -- bad hash
    let err = runner
        .wrbsite_load_ext(
            &mut mock_bns_resolver,
            "sad-3",
            "path",
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
        )
        .unwrap_err();
    assert!(matches!(err, Error::FailedToRun(..)));

    // sad path -- no such name
    let err = runner
        .wrbsite_load_ext(
            &mut mock_bns_resolver,
            "nonexistant",
            "path",
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
            |_, _| Ok(Box::new(mock_stackerdb.clone())),
        )
        .unwrap_err();
    assert!(matches!(err, Error::FailedToRun(..)));
}
