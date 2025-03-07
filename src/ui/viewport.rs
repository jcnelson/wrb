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

use clarity::vm::Value;
use std::collections::HashMap;
use std::fmt;

use crate::ui::charbuff::CharBuff;
use crate::ui::charbuff::CharCell;
use crate::ui::charbuff::Color;
use crate::ui::Error;

#[derive(Clone, PartialEq)]
pub struct Viewport {
    pub id: u128,
    /// position in the root
    start_row: u64,
    start_col: u64,
    /// number of rows that will be visible
    /// (num_cols in in `buff`)
    num_rows: u64,
    /// viewport row offset
    scroll_offset: u64,
    /// is it available for rendering?
    visible: bool,
    /// parent viewport
    pub(crate) parent: Option<u128>,
    /// previously-inserted viewport (used for loading from clarity)
    pub(crate) prev_viewport: Option<u128>,
    /// contents
    buff: CharBuff,
    /// upper-left corners of each UI element
    element_coords: HashMap<u128, (u64, u64)>,
}

impl fmt::Debug for Viewport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Viewport({},({},{}),({},{}),scroll={},visible={},buff={})",
            self.id,
            self.start_row,
            self.start_col,
            self.start_row + self.num_rows,
            self.start_col + self.buff.num_cols,
            self.scroll_offset,
            self.visible,
            self.buff.cells.len()
        )
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

        let start_row = u64::try_from(
            viewport_tuple
                .get("start-row")
                .cloned()
                .expect("FATAL: no start-row")
                .expect_u128()?,
        )
        .expect("too many rows");

        let start_col = u64::try_from(
            viewport_tuple
                .get("start-col")
                .cloned()
                .expect("FATAL: no start-col")
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

        let num_cols = u64::try_from(
            viewport_tuple
                .get("num-cols")
                .cloned()
                .expect("FATAL: no num-cols")
                .expect_u128()?,
        )
        .expect("too many columns");

        let visible = viewport_tuple
            .get("visible")
            .cloned()
            .expect("FATAL: no visible")
            .expect_bool()?;

        let last_opt = viewport_tuple
            .get("last")
            .cloned()
            .expect("FATAL: no `last`")
            .expect_optional()?
            .map(|last_value| last_value.expect_u128())
            .transpose()?;

        let parent_opt = viewport_tuple
            .get("parent")
            .cloned()
            .expect("FATAL: no `last`")
            .expect_optional()?
            .map(|last_value| last_value.expect_u128())
            .transpose()?;

        Ok(Viewport {
            id,
            start_row,
            start_col,
            num_rows,
            scroll_offset: 0,
            visible,
            prev_viewport: last_opt,
            parent: parent_opt,
            buff: CharBuff::new(num_cols),
            element_coords: HashMap::new(),
        })
    }

    pub fn new(id: u128, start_row: u64, start_col: u64, num_rows: u64, num_cols: u64) -> Viewport {
        Viewport {
            id,
            start_row,
            start_col,
            num_rows,
            scroll_offset: 0,
            visible: true,
            prev_viewport: None,
            parent: None,
            buff: CharBuff::new(num_cols),
            element_coords: HashMap::new(),
        }
    }

    pub fn new_child(
        id: u128,
        parent_id: u128,
        start_row: u64,
        start_col: u64,
        num_rows: u64,
        num_cols: u64,
    ) -> Viewport {
        Viewport {
            id,
            start_row,
            start_col,
            num_rows,
            scroll_offset: 0,
            visible: true,
            prev_viewport: None,
            parent: Some(parent_id),
            buff: CharBuff::new(num_cols),
            element_coords: HashMap::new(),
        }
    }

    /// What's the start row/col of this viewport?
    pub fn pos(&self) -> (u64, u64) {
        (self.start_row, self.start_col)
    }

    /// What are the dimensions of this viewport?
    pub fn dims(&self) -> (u64, u64) {
        (self.num_rows, self.buff.num_cols)
    }

    /// Update the coordinate of a UI element
    fn update_element_coord(&mut self, element_id: u128, start_row: u64, start_col: u64) {
        if let Some((r, c)) = self.element_coords.get_mut(&element_id) {
            *r = (*r).min(start_row);
            *c = (*c).min(start_col);
        } else {
            self.element_coords
                .insert(element_id, (start_row, start_col));
        }
    }

    /// Write text to this viewport.
    /// `start_col` and `start_row` are coordinates within the viewport.
    pub fn print_to(
        &mut self,
        element_id: u128,
        start_row: u64,
        start_col: u64,
        bg_color: Color,
        fg_color: Color,
        text: &str,
    ) -> (u64, u64) {
        self.update_element_coord(element_id, start_row, start_col);
        self.buff
            .print_at(element_id, start_row, start_col, bg_color, fg_color, text)
    }

    /// Write word-wrapped text to this viewport
    /// `start_col` and `start_row` are coordinates within the viewport
    pub fn print(
        &mut self,
        element_id: u128,
        start_row: u64,
        start_col: u64,
        bg_color: Color,
        fg_color: Color,
        text: &str,
    ) -> (u64, u64) {
        self.update_element_coord(element_id, start_row, start_col);
        self.buff
            .print(element_id, start_row, start_col, bg_color, fg_color, text)
    }

    /// Write word-wrapped text to this viewport, with a newline at the end
    /// `start_col` and `start_row` are coordinates within the viewport
    pub fn println(
        &mut self,
        element_id: u128,
        start_row: u64,
        start_col: u64,
        bg_color: Color,
        fg_color: Color,
        text: &str,
    ) -> (u64, u64) {
        self.update_element_coord(element_id, start_row, start_col);
        self.buff
            .println(element_id, start_row, start_col, bg_color, fg_color, text)
    }

    /// Write word-wrapped text to this viewport from a char iterator.
    /// attempts to word-wrap.
    /// `start_col` and `start_row` are coordinates within the viewport
    pub fn print_iter(
        &mut self,
        element_id: u128,
        start_row: u64,
        start_col: u64,
        bg_color: Color,
        fg_color: Color,
        iter: impl Iterator<Item = char>,
    ) -> (u64, u64) {
        self.update_element_coord(element_id, start_row, start_col);
        self.buff
            .print_iter(element_id, start_row, start_col, bg_color, fg_color, iter)
    }

    /// Write word-wrapped text to this viewport from a char iterator.
    /// `start_col` and `start_row` are coordinates within the viewport
    pub fn print_at_iter(
        &mut self,
        element_id: u128,
        start_row: u64,
        start_col: u64,
        bg_color: Color,
        fg_color: Color,
        iter: impl Iterator<Item = char>,
    ) -> (u64, u64) {
        self.update_element_coord(element_id, start_row, start_col);
        self.buff
            .print_at_iter(element_id, start_row, start_col, bg_color, fg_color, iter)
    }

    /// What's the relative (row, column) coordinate in this viewport, given the absolute
    /// (row,col) coordinate and the viewport's absolute (row,col) coordinate?
    pub fn translate_coordinate(
        &self,
        viewport_abs_row: u64,
        viewport_abs_col: u64,
        abs_row: u64,
        abs_col: u64,
    ) -> Option<(u64, u64)> {
        let (dim_rows, dim_cols) = self.dims();

        // bounding box check
        if abs_row < self.start_row + viewport_abs_row {
            return None;
        }
        if abs_row >= self.start_row + viewport_abs_row + dim_rows {
            return None;
        }
        if abs_col < self.start_col + viewport_abs_col {
            return None;
        }
        if abs_col >= self.start_col + viewport_abs_col + dim_cols {
            return None;
        }

        // fits in bounding box
        Some((
            abs_row.checked_sub(self.start_row + viewport_abs_row)?,
            abs_col.checked_sub(self.start_col + viewport_abs_col)?,
        ))
    }

    /// What's the relative (row, column) coordinate of a UI element in this viewport, given a
    /// viewport-relative (row, column) coordinate?
    /// Return Some((element_id, row, col)) if the given viewport-relative coordinates fall onto a UI element.
    /// Return None otherwise
    pub fn get_ui_coordinate(
        &self,
        viewport_abs_row: u64,
        viewport_abs_col: u64,
    ) -> Option<(u128, u64, u64)> {
        let Some(cell) = self.buff.charcell_at(viewport_abs_row, viewport_abs_col) else {
            return None;
        };
        let CharCell::Fill {
            value: _value,
            bg: _bg,
            fg: _fg,
            element_id,
        } = cell
        else {
            return None;
        };
        let Some((ui_row, ui_col)) = self.element_coords.get(&element_id) else {
            // shouldn't be possible
            wrb_warn!(
                "Potentially unreachable: no coordinate for element {}",
                &element_id
            );
            return None;
        };
        Some((
            element_id,
            viewport_abs_row.checked_sub(*ui_row)?,
            viewport_abs_col.checked_sub(*ui_col)?,
        ))
    }

    /// What's the charcell at the given relative coordinates, when taking into account scrolling?
    pub fn charcell_at(&self, rel_row: u64, rel_col: u64) -> Option<CharCell> {
        if rel_row + self.scroll_offset >= self.num_rows {
            return None;
        }
        self.buff.charcell_at(rel_row + self.scroll_offset, rel_col)
    }

    /// Are we visible?
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Set visible
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Clear the inner char buff
    pub fn clear(&mut self) {
        self.buff.clear()
    }
}
