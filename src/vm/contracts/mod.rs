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
pub const WRB_LL_CODE: &'static str = std::include_str!("wrb-ll.clar");
const WRB_CODE: &'static str = std::include_str!("wrb.clar");

pub fn wrb_link_app(app_code: &str) -> String {
    format!(
        r#"{}
;; ================= END OF WRBLIB =======================
{}"#,
        WRB_CODE, app_code
    )
}
