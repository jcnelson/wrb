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

use crate::runner::site::{WrbTxtRecord, WrbTxtRecordV1, ZonefileResourceRecord};
use crate::runner::Runner;

use libstackerdb::SlotMetadata;

use clarity::vm::types::QualifiedContractIdentifier;

use base64ct::{Base64, Encoding};

use crate::stacks_common::codec::StacksMessageCodec;
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
            rr_payload: vec!["1.2.3.4".into()],
        },
        ZonefileResourceRecord {
            rr_name: "baz".into(),
            rr_ttl: None,
            rr_class: "IN".into(),
            rr_type: "A".into(),
            rr_payload: vec!["5.6.7.8".into()],
        },
        ZonefileResourceRecord {
            rr_name: "_http._tcp".into(),
            rr_ttl: Some(3600),
            rr_class: "IN".into(),
            rr_type: "TXT".into(),
            rr_payload: vec!["\"http://example.com\"".into()],
        },
        ZonefileResourceRecord {
            rr_name: "wrb".into(),
            rr_ttl: None,
            rr_class: "IN".into(),
            rr_type: "TXT".into(),
            rr_payload: vec!["\"asdffdsa\"".into()],
        },
        ZonefileResourceRecord {
            rr_name: "wrb".into(),
            rr_ttl: None,
            rr_class: "IN".into(),
            rr_type: "TXT".into(),
            rr_payload: vec!["\"asdffdsa\"".into(), "\"quux\"".into()],
        },
    ];

    let zonefile = b"$ORIGIN foo.com\nbar 3600 IN A 1.2.3.4\nbaz IN A 5.6.7.8\n_http._tcp 3600 IN TXT \"http://example.com\"\nwrb IN TXT \"asdffdsa\"\nwrb IN TXT \"asdffdsa\" \"quux\"".to_vec();
    let rrs = Runner::decode_zonefile_records(zonefile).unwrap();
    assert_eq!(rrs, expected_rrs);

    let zonefile = b"$ORIGIN foo.com\n\nbar \t3600 IN\tA 1.2.3.4\nbaz\t\t\tIN        A \t \t 5.6.7.8\n_http._tcp \t 3600     IN \t\t\t  \t TXT \"http://example.com\"\nwrb \t\t\t IN\t \tTXT\t \"asdffdsa\"\nwrb IN TXT \"asdffdsa\" \t\t\t \"quux\"\n\n\n    \n".to_vec();
    let rrs = Runner::decode_zonefile_records(zonefile).unwrap();
    assert_eq!(rrs, expected_rrs);

    let zonefile = b"$ORIGIN foo.com\nbad1 IN\nbar 3600 IN A 1.2.3.4\nbad2 IN A\nbaz IN A 5.6.7.8\nbad3 3600\n_http._tcp 3600 IN TXT \"http://example.com\"\nbad4 \"bad4\"\nwrb IN TXT \"asdffdsa\"\nwrb IN TXT \"asdffdsa\" \"quux\"".to_vec();
    let rrs = Runner::decode_zonefile_records(zonefile).unwrap();
    assert_eq!(rrs, expected_rrs);
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
        format!("wrb\t\tIN\tTXT\t\"{}\" ", &b64)
    );
}
