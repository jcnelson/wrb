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

use std::fmt;
use clarity::vm::Value;

use crate::ui::charbuff::CharBuff;
use crate::ui::charbuff::CharCell;
use crate::ui::charbuff::Color;
use crate::ui::Error;

pub struct Viewport {
    pub id: u128,
    /// position in the root
    start_col: u64,
    start_row: u64,
    /// number of rows that will be visible
    /// (num_cols in in `buff`)
    num_rows: u64,
    /// viewport row offset
    scroll_offset: u64,
    /// is it avvailable for rendering?
    visible: bool,
    /// contents
    buff: CharBuff,
}

impl fmt::Debug for Viewport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Viewport(({},{}),({},{}),scroll={},visible={},buff={})", self.start_col, self.start_row, self.start_col + self.buff.num_cols, self.start_row + self.num_rows, self.scroll_offset, self.visible, self.buff.cells.len())
    }
}

impl Viewport {
    /// This is hard-coded from the viewport tuple definition in .wrb-ll
    pub fn from_clarity_value(value: Value) -> Result<Viewport, Error> {
        let viewport_tuple = value.expect_tuple()?;
        let id = viewport_tuple
            .get("id")
            .cloned()
            .expect("FATAL: no id")
            .expect_u128()?;

        let start_col = u64::try_from(
            viewport_tuple
                .get("start-col")
                .cloned()
                .expect("FATAL: no start-col")
                .expect_u128()?,
        )
        .expect("too many columns");

        let start_row = u64::try_from(
            viewport_tuple
                .get("start-row")
                .cloned()
                .expect("FATAL: no start-row")
                .expect_u128()?,
        )
        .expect("too many rows");

        let num_cols = u64::try_from(
            viewport_tuple
                .get("num-cols")
                .cloned()
                .expect("FATAL: no num-cols")
                .expect_u128()?,
        )
        .expect("too many columns");

        let num_rows = u64::try_from(
            viewport_tuple
                .get("num-rows")
                .cloned()
                .expect("FATAL: no num-rows")
                .expect_u128()?,
        )
        .expect("too many rows");

        let visible = viewport_tuple
            .get("visible")
            .cloned()
            .expect("FATAL: no visible")
            .expect_bool()?;

        Ok(Viewport {
            id,
            start_col,
            start_row,
            num_rows,
            scroll_offset: 0,
            visible,
            buff: CharBuff::new(num_cols),
        })
    }

    pub fn new(id: u128, start_col: u64, start_row: u64, num_cols: u64, num_rows: u64) -> Viewport {
        Viewport {
            id,
            start_col,
            start_row,
            num_rows,
            scroll_offset: 0,
            visible: true,
            buff: CharBuff::new(num_cols)
        }
    }

    /// What are the dimensions of this viewport?
    /// Returns (num-cols, num-rows)
    pub fn dims(&self) -> (u64, u64) {
        (self.num_rows, self.buff.num_cols)
    }

    /// Write text to this viewport.
    /// `start_col` and `start_row` are coordinates within the viewport.
    pub fn print_to(
        &mut self,
        start_col: u64,
        start_row: u64,
        bg_color: Color,
        fg_color: Color,
        text: &str,
    ) -> (u64, u64) {
        self.buff
            .print_at(start_col, start_row, bg_color, fg_color, text)
    }

    /// Write word-wrapped text to this viewport
    /// `start_col` and `start_row` are coordinates within the viewport
    pub fn print(
        &mut self,
        start_col: u64,
        start_row: u64,
        bg_color: Color,
        fg_color: Color,
        text: &str,
    ) -> (u64, u64) {
        self.buff
            .print(start_col, start_row, bg_color, fg_color, text)
    }

    /// Write word-wrapped text to this viewport, with a newline at the end
    /// `start_col` and `start_row` are coordinates within the viewport
    pub fn println(
        &mut self,
        start_col: u64,
        start_row: u64,
        bg_color: Color,
        fg_color: Color,
        text: &str,
    ) -> (u64, u64) {
        self.buff
            .println(start_col, start_row, bg_color, fg_color, text)
    }


    /// What's the relative (column, row) coordinate in this viewport, given the absolute
    /// (column,row) coordinate?
    pub fn translate_coordinate(&self, abs_col: u64, abs_row: u64) -> Option<(u64, u64)> {
        let (dim_rows, dim_cols) = self.dims();

        // bounding box check
        if abs_col < self.start_col {
            return None;
        }
        if abs_col >= self.start_col + dim_cols {
            return None;
        }
        if abs_row < self.start_row {
            return None;
        }
        if abs_row >= self.start_row + dim_rows {
            return None;
        }

        // fits in bounding box
        Some((
            abs_col.checked_sub(self.start_col).unwrap(),
            abs_row.checked_sub(self.start_row).unwrap(),
        ))
    }

    /// What's the charcell at the given relative coordinates, when taking into account scrolling?
    pub fn charcell_at(&self, rel_col: u64, rel_row: u64) -> Option<CharCell> {
        if rel_row + self.scroll_offset >= self.num_rows {
            return None;
        }
        self.buff.charcell_at(rel_col, rel_row + self.scroll_offset)
    }

    /// Are we visible?
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Set visible
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}
