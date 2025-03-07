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
use crate::core;
use crate::core::with_globals;

use crate::cli::subcommand_clarity;

use crate::stacks_common::codec::StacksMessageCodec;
use stacks_common::util::hash::to_hex;

#[test]
fn test_json_to_clarity() {
    let json_str = r#"{ "a": 1, "b": "\"hello world\"", "c": { "d": false, "e": null, "f": [ "u\"ghij\"", "u\"klm\"", "u\"n\"" ] }, "g": "u1", "h": "(+ 1 2)", "i": -1}"#;
    let val = json_to_clarity(&mut json_str.as_bytes()).unwrap();

    eprintln!("{:?}", &val);

    let val_tuple = val.expect_tuple().unwrap();
    let val_a = val_tuple.get("a").cloned().unwrap().expect_i128().unwrap();
    assert_eq!(val_a, 1);

    let val_b = val_tuple.get("b").cloned().unwrap().expect_ascii().unwrap();
    assert_eq!(val_b, "hello world");

    let val_c_tuple = val_tuple.get("c").cloned().unwrap().expect_tuple().unwrap();
    let val_d = val_c_tuple
        .get("d")
        .cloned()
        .unwrap()
        .expect_bool()
        .unwrap();
    assert_eq!(val_d, false);

    let val_e = val_c_tuple
        .get("e")
        .cloned()
        .unwrap()
        .expect_optional()
        .unwrap();
    assert_eq!(val_e, None);

    let val_f = val_c_tuple
        .get("f")
        .cloned()
        .unwrap()
        .expect_list()
        .unwrap();
    for (i, val) in val_f.into_iter().enumerate() {
        let val_str = val.expect_utf8().unwrap();

        if i == 0 {
            assert_eq!(val_str, "ghij");
        }
        if i == 1 {
            assert_eq!(val_str, "klm");
        }
        if i == 2 {
            assert_eq!(val_str, "n");
        }
    }

    let val_g = val_tuple.get("g").cloned().unwrap().expect_u128().unwrap();
    assert_eq!(val_g, 1);

    let val_h = val_tuple.get("h").cloned().unwrap().expect_i128().unwrap();
    assert_eq!(val_h, 3);

    let val_i = val_tuple.get("i").cloned().unwrap().expect_i128().unwrap();
    assert_eq!(val_i, -1);

    // this will fail due to incompatible list types
    let json_str = r#"[ 1, false, "abc"]"#;
    assert!(json_to_clarity(&mut json_str.as_bytes()).is_err());
}
