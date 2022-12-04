// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2022 Stacks Open Internet Foundation
// Copyright (C) 2022 Jude Nelson
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
use crate::vm::ClarityVM;

#[test]
fn test_vm_bootup() {
    let db_path = "/tmp/wrb-clarity-vm-test-bootup";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let _ = ClarityVM::new(db_path, "foo.btc");
} 
