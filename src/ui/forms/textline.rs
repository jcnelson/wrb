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

use termion::event::Key;

/// UI command to add an editable line of text
#[derive(Clone, PartialEq, Debug)]
pub struct TextLine {
    element_id: u128,
    viewport_id: u128,
    row: u64,
    col: u64,
    cursor: usize,
    bg_color: Color,
    fg_color: Color,
    focused_bg_color: Color,
    focused_fg_color: Color,
    inner_text: String,
    max_len: usize,
    insert: bool,
}

impl TextLine {
    /// Constructor for consumers who just want to use the event handler to manipulate the text
    pub fn new_detached(text: String, max_len: usize) -> Self {
        Self {
            element_id: 0,
            viewport_id: 0,
            row: 0,
            col: 0,
            cursor: 0,
            bg_color: 0u32.into(),
            fg_color: 0xffffffu32.into(),
            focused_bg_color: 0xffffffu32.into(),
            focused_fg_color: 0u32.into(),
            inner_text: text,
            max_len,
            insert: true,
        }
    }

    pub fn text(&self) -> &str {
        self.inner_text.as_str()
    }

    pub fn set_text(&mut self, txt: String) {
        self.inner_text = txt;
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn insert(&self) -> bool {
        self.insert
    }
}

impl WrbForm for TextLine {
    fn type_id(&self) -> WrbFormTypes {
        WrbFormTypes::TextLine
    }

    fn element_id(&self) -> u128 {
        self.element_id
    }

    fn viewport_id(&self) -> u128 {
        self.viewport_id
    }

    fn focus(&mut self, root: &mut Root, focused: bool) -> Result<(), Error> {
        if focused {
            root.set_form_cursor(
                self.viewport_id,
                self.row,
                self.col + u64::try_from(self.cursor).unwrap_or(0),
            );
        }
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

        let max_len = text_tuple
            .get("max-len")
            .cloned()
            .expect("FATAL: no `max-len`")
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

        let focused_bg_color_u128 = text_tuple
            .get("focused-bg-color")
            .cloned()
            .expect("FATAL: no `focused-bg-color`")
            .expect_u128()?
            // truncate
            & 0xffffffffu128;

        let focused_fg_color_u128 = text_tuple
            .get("focused-fg-color")
            .cloned()
            .expect("FATAL: no `focused-fg-color`")
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
        let focused_bg_color: Color = u32::try_from(focused_bg_color_u128)
            .expect("infallible")
            .into();
        let focused_fg_color: Color = u32::try_from(focused_fg_color_u128)
            .expect("infallible")
            .into();

        Ok(TextLine {
            element_id,
            viewport_id,
            row: u64::try_from(row).map_err(|_| Error::Codec("row too big".into()))?,
            col: u64::try_from(col).map_err(|_| Error::Codec("col too big".into()))?,
            cursor: 0,
            max_len: usize::try_from(max_len)
                .map_err(|_| Error::Codec("max-len too big".into()))?,
            bg_color,
            fg_color,
            focused_bg_color,
            focused_fg_color,
            inner_text: text,
            insert: true,
        })
    }

    /// Return the actionable data
    fn to_clarity_value(&self) -> Result<Option<Value>, Error> {
        let value_opt = Value::string_utf8_from_string_utf8_literal(self.inner_text.clone())
            .map_err(|e| {
                wrb_warn!("Failed to convert inner text of textline element {} in viewport {} into a Clarity value: {:?}", self.element_id, self.viewport_id, &e);
                e
            })
            .ok();

        Ok(value_opt)
    }

    fn render(&mut self, root: &mut Root, _cursor: (u64, u64)) -> Result<(u64, u64), Error> {
        let focused = root.is_focused(self.element_id);
        let Some(viewport) = root.viewport_mut(self.viewport_id) else {
            return Err(Error::NoViewport(self.viewport_id));
        };
        let bg_color = if focused {
            self.focused_bg_color.clone()
        } else {
            self.bg_color.clone()
        };
        let fg_color = if focused {
            self.focused_fg_color.clone()
        } else {
            self.fg_color.clone()
        };

        let (_vp_rows, vp_cols) = viewport.dims();
        let max_viewable_cols = vp_cols
            .saturating_sub(self.col)
            .min(u64::try_from(self.max_len).unwrap_or(u64::MAX));
        let padded_text = format!(
            "{:width$}",
            &self.inner_text,
            width = usize::try_from(max_viewable_cols).unwrap_or(0)
        );
        let new_cursor = viewport.print_to(
            self.element_id,
            self.row,
            self.col,
            bg_color,
            fg_color,
            &padded_text,
        );

        // set the form cursor to be wherever our cursor is
        if focused {
            root.set_form_cursor(
                self.viewport_id,
                self.row,
                self.col + u64::try_from(self.cursor).unwrap_or(0),
            );
        }
        Ok(new_cursor)
    }

    /// This doesn't generate an event the main loop cares about, but it does update the text
    /// buffer.
    fn handle_event(
        &mut self,
        root: &mut Root,
        event: WrbFormEvent,
    ) -> Result<Option<Value>, Error> {
        match event {
            WrbFormEvent::Keypress(key) => match key {
                Key::Left => {
                    self.cursor = self.cursor.saturating_sub(1);
                }
                Key::Right => {
                    self.cursor = self
                        .cursor
                        .saturating_add(1)
                        .min(self.inner_text.len())
                        .min(self.max_len);
                }
                Key::Backspace | Key::Ctrl('h') => {
                    if self.cursor > 0 {
                        let mut new_text = String::with_capacity(self.max_len);
                        for (i, chr) in self.inner_text.chars().enumerate() {
                            if i == self.cursor - 1 {
                                continue;
                            }
                            new_text.push(chr);
                        }
                        self.inner_text = new_text;
                        self.cursor -= 1;
                    }
                }
                Key::Delete | Key::Ctrl('?') => {
                    if self.cursor < self.inner_text.len() {
                        let mut new_text = String::with_capacity(self.max_len);
                        for (i, chr) in self.inner_text.chars().enumerate() {
                            if i == self.cursor {
                                continue;
                            }
                            new_text.push(chr);
                        }
                        self.inner_text = new_text;
                    }
                }
                Key::Insert => {
                    self.insert = !self.insert;
                }
                Key::Home => {
                    self.cursor = 0;
                }
                Key::End => {
                    self.cursor = self.inner_text.len();
                }
                Key::Char(c) => {
                    if c != '\n' && c != '\r' {
                        if self.cursor == self.inner_text.len()
                            && self.inner_text.len() < self.max_len
                        {
                            self.inner_text.push(c);
                            self.cursor += 1;
                        } else if self.inner_text.len() < self.max_len {
                            let mut new_text = String::with_capacity(self.max_len);
                            for (i, chr) in self.inner_text.chars().enumerate() {
                                if i == self.cursor {
                                    new_text.push(c);
                                    if !self.insert {
                                        continue;
                                    }
                                }
                                new_text.push(chr);
                            }
                            self.inner_text = new_text;
                            self.cursor += 1;
                        }
                    }
                }
                _ => {}
            },
        }
        self.focus(root, root.is_focused(self.element_id))?;
        Ok(None)
    }
}
