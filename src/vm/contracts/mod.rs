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
pub const WRB_CONTRACT: &'static str = "wrb";
pub const WRB_LOW_LEVEL_CONTRACT: &'static str = "wrb-ll";

const WRB_LOW_LEVEL_CODE : &'static str = std::include_str!("wrb-ll.clar");
const WRB_CODE : &'static str = std::include_str!("wrb.clar");
pub const WRBLIB_CODE : &'static str = std::include_str!("wrblib.clar");

pub const BOOT_CODE: &'static [(&'static str, &'static str)] = &[
    (
        WRB_LOW_LEVEL_CONTRACT,
        WRB_LOW_LEVEL_CODE,
    ),
    (
        WRB_CONTRACT,
        WRB_CODE,
    ),
];
