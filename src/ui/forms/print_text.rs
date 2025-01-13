// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2023 Stacks Open Internet Foundation
// Copyright (C) 2024 Jude Nelson
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

use crate::ui::charbuff::Color;
use crate::ui::root::Root;
use crate::ui::Error;
use crate::ui::ValueExtensions;
use clarity::vm::Value;

use crate::ui::forms::{WrbForm, WrbFormEvent, WrbFormTypes};

/// UI command to print text to a viewport
#[derive(Clone, PartialEq, Debug)]
pub struct PrintText {
    element_id: u128,
    viewport_id: u128,
    // (row, column)
    cursor: Option<(u64, u64)>,
    bg_color: Color,
    fg_color: Color,
    text: String,
    newline: bool,
}

impl WrbForm for PrintText {
    fn type_id(&self) -> WrbFormTypes {
        WrbFormTypes::Print
    }

    fn element_id(&self) -> u128 {
        self.element_id
    }

    fn viewport_id(&self) -> u128 {
        self.viewport_id
    }

    fn focus(&mut self, _root: &mut Root, _focus: bool) -> Result<(), Error> {
        Ok(())
    }

    /// Load from Clarity value
    fn from_clarity_value(viewport_id: u128, v: Value) -> Result<Self, Error> {
        let text_tuple = v.expect_tuple()?;
        let text = text_tuple
            .get("text")
            .cloned()
            .expect("FATAL: no `text`")
            .expect_utf8()?;

        let cursor = match text_tuple
            .get("cursor")
            .cloned()
            .expect("FATAL: no `cursor`")
            .expect_optional()?
        {
            Some(cursor_tuple_value) => {
                let cursor_tuple = cursor_tuple_value.expect_tuple()?;
                let row = cursor_tuple
                    .get("row")
                    .cloned()
                    .expect("FATAL: no `row`")
                    .expect_u128()?;
                let col = cursor_tuple
                    .get("col")
                    .cloned()
                    .expect("FATAL: no `col`")
                    .expect_u128()?;
                Some((
                    u64::try_from(row).map_err(|_| Error::Codec("row too big".into()))?,
                    u64::try_from(col).map_err(|_| Error::Codec("col too big".into()))?,
                ))
            }
            None => None,
        };

        let bg_color_u128 = text_tuple
            .get("bg-color")
            .cloned()
            .expect("FATAL: no `bg-color`")
            .expect_u128()?
            // truncate
            & 0xffffffffu128;

        let fg_color_u128 = text_tuple
            .get("fg-color")
            .cloned()
            .expect("FATAL: no `fg-color`")
            .expect_u128()?
            // trunate
            &0xffffffffu128;

        let element_id = text_tuple
            .get("element-id")
            .cloned()
            .expect("FATAL: no `element-id`")
            .expect_u128()?;

        let newline = text_tuple
            .get("newline")
            .cloned()
            .expect("FATAL: no `newline`")
            .expect_bool()?;

        let bg_color: Color = u32::try_from(bg_color_u128).expect("infallible").into();
        let fg_color: Color = u32::try_from(fg_color_u128).expect("infallible").into();

        Ok(PrintText {
            element_id,
            viewport_id,
            cursor,
            bg_color,
            fg_color,
            text,
            newline,
        })
    }

    fn to_clarity_value(&self) -> Result<Option<Value>, Error> {
        Ok(None)
    }

    /// Render to Root
    /// cursor is (row, col)
    fn render(&mut self, root: &mut Root, cursor: (u64, u64)) -> Result<(u64, u64), Error> {
        let Some(viewport) = root.viewport_mut(self.viewport_id) else {
            return Err(Error::NoViewport(self.viewport_id));
        };
        let cursor = self.cursor.clone().unwrap_or(cursor);
        wrb_test_debug!("Print '{}' at {:?}", &self.text, &cursor);
        if self.newline {
            Ok(viewport.println(
                self.element_id,
                cursor.0,
                cursor.1,
                self.bg_color,
                self.fg_color,
                &self.text,
            ))
        } else {
            Ok(viewport.print(
                self.element_id,
                cursor.0,
                cursor.1,
                self.bg_color,
                self.fg_color,
                &self.text,
            ))
        }
    }

    fn handle_event(
        &mut self,
        _root: &mut Root,
        _event: WrbFormEvent,
    ) -> Result<Option<Value>, Error> {
        Ok(None)
    }
}
