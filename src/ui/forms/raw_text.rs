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

/// UI command to add text to a viewport
#[derive(Clone, PartialEq, Debug)]
pub struct RawText {
    element_id: u128,
    viewport_id: u128,
    row: u64,
    col: u64,
    bg_color: Color,
    fg_color: Color,
    text: String,
}

impl WrbForm for RawText {
    fn type_id(&self) -> WrbFormTypes {
        WrbFormTypes::Text
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

    /// construct from Clarity value
    fn from_clarity_value(viewport_id: u128, v: Value) -> Result<Self, Error> {
        let text_tuple = v.expect_tuple()?;
        let text = text_tuple
            .get("text")
            .cloned()
            .expect("FATAL: no `text`")
            .expect_utf8()?;

        let row = text_tuple
            .get("row")
            .cloned()
            .expect("FATAL: no `row`")
            .expect_u128()?;

        let col = text_tuple
            .get("col")
            .cloned()
            .expect("FATAL: no `col`")
            .expect_u128()?;

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

        let bg_color: Color = u32::try_from(bg_color_u128).expect("infallible").into();
        let fg_color: Color = u32::try_from(fg_color_u128).expect("infallible").into();

        Ok(RawText {
            element_id,
            viewport_id,
            row: u64::try_from(row).map_err(|_| Error::Codec("row too big".into()))?,
            col: u64::try_from(col).map_err(|_| Error::Codec("col too big".into()))?,
            bg_color,
            fg_color,
            text,
        })
    }

    // No state to store for raw text
    fn to_clarity_value(&self) -> Result<Option<Value>, Error> {
        Ok(None)
    }

    fn render(&mut self, root: &mut Root, _cursor: (u64, u64)) -> Result<(u64, u64), Error> {
        let Some(viewport) = root.viewport_mut(self.viewport_id) else {
            return Err(Error::NoViewport(self.viewport_id));
        };
        let new_cursor = viewport.print_to(
            self.element_id,
            self.row,
            self.col,
            self.bg_color,
            self.fg_color,
            &self.text,
        );
        Ok(new_cursor)
    }

    fn handle_event(
        &mut self,
        _root: &mut Root,
        _event: WrbFormEvent,
    ) -> Result<Option<Value>, Error> {
        Ok(None)
    }
}
