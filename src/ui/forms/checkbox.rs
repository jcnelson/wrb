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

use clarity::vm::types::TupleData;
use clarity::vm::Value;

use crate::ui::charbuff::Color;
use crate::ui::root::Root;
use crate::ui::Error;
use crate::ui::ValueExtensions;

use crate::ui::forms::{WrbForm, WrbFormEvent, WrbFormTypes};

use crate::DEFAULT_WRB_EPOCH;

#[derive(Clone, PartialEq, Debug)]
struct CheckboxOption {
    text: String,
    selected: bool,
}

impl CheckboxOption {
    pub fn from_clarity_value(value: Value) -> Result<Self, Error> {
        let checkbox_option_tuple = value.expect_tuple()?;
        let text = checkbox_option_tuple
            .get("text")
            .cloned()
            .expect("FATAL: no `text`")
            .expect_utf8()?;

        let selected = checkbox_option_tuple
            .get("selected")
            .cloned()
            .expect("FATAL: no `selected`")
            .expect_bool()?;

        Ok(Self { text, selected })
    }

    pub fn to_clarity_value(&self) -> Value {
        Value::Tuple(
            TupleData::from_data(vec![
                (
                    "text".into(),
                    Value::string_utf8_from_string_utf8_literal(self.text.clone())
                        .expect("FATAL: could not convert UTF-8 literal back to Clarity string"),
                ),
                ("selected".into(), Value::Bool(self.selected)),
            ])
            .expect("FATAL: could not build tuple from checkbox item"),
        )
    }

    pub fn to_string(&self) -> String {
        format!("[{}] {}", if self.selected { "*" } else { " " }, &self.text)
    }
}

/// UI command to produce a sequence of checkboxed values
#[derive(Clone, PartialEq, Debug)]
pub struct Checkbox {
    element_id: u128,
    viewport_id: u128,
    row: u64,
    col: u64,
    bg_color: Color,
    fg_color: Color,
    focused_bg_color: Color,
    focused_fg_color: Color,
    selector_color: Color,
    options: Vec<CheckboxOption>,
    selector: usize,
}

pub const CHECKBOX_MAX_LEN: usize = 256;

impl WrbForm for Checkbox {
    /// type
    fn type_id(&self) -> WrbFormTypes {
        WrbFormTypes::Checkbox
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
                self.row + u64::try_from(self.selector).unwrap_or(0),
                self.col + 1,
            );
        }
        Ok(())
    }

    /// Load from a Clarity value
    fn from_clarity_value(viewport_id: u128, v: Value) -> Result<Self, Error> {
        let checkbox_tuple = v.expect_tuple()?;
        let row = u64::try_from(
            checkbox_tuple
                .get("row")
                .cloned()
                .expect("FATAL: no `row`")
                .expect_u128()?,
        )
        .map_err(|_| Error::Codec("row is too big".into()))?;

        let col = u64::try_from(
            checkbox_tuple
                .get("col")
                .cloned()
                .expect("FATAL: no `col`")
                .expect_u128()?,
        )
        .map_err(|_| Error::Codec("col is too big".into()))?;

        let bg_color_u128 = checkbox_tuple
            .get("bg-color")
            .cloned()
            .expect("FATAL: no `bg-color`")
            .expect_u128()?
            // truncate
            & 0xffffffffu128;

        let fg_color_u128 = checkbox_tuple
            .get("fg-color")
            .cloned()
            .expect("FATAL: no `fg-color`")
            .expect_u128()?
            // trunate
            &0xffffffffu128;

        let focused_bg_color_u128 = checkbox_tuple
            .get("focused-bg-color")
            .cloned()
            .expect("FATAL: no `focused-bg-color`")
            .expect_u128()?
            // truncate
            & 0xffffffffu128;

        let focused_fg_color_u128 = checkbox_tuple
            .get("focused-fg-color")
            .cloned()
            .expect("FATAL: no `focused-fg-color`")
            .expect_u128()?
            // trunate
            &0xffffffffu128;

        let selector_color_u128 = checkbox_tuple
            .get("selector-color")
            .cloned()
            .expect("FATAL: no `selector-color`")
            .expect_u128()?
            // trunate
            &0xffffffffu128;

        let element_id = checkbox_tuple
            .get("element-id")
            .cloned()
            .expect("FATAL: no `element-id`")
            .expect_u128()?;

        let text_options_list = checkbox_tuple
            .get("options")
            .cloned()
            .expect("FATAL: no `options`")
            .expect_list()?;

        if text_options_list.len() > CHECKBOX_MAX_LEN {
            return Err(Error::Page(format!(
                "Too many checkbox items (max is {}, but got {})",
                CHECKBOX_MAX_LEN,
                text_options_list.len()
            )));
        }

        let mut options = vec![];
        for option_value in text_options_list.into_iter() {
            let checkbox_option = CheckboxOption::from_clarity_value(option_value)?;
            options.push(checkbox_option);
        }

        let bg_color: Color = u32::try_from(bg_color_u128).expect("infallible").into();
        let fg_color: Color = u32::try_from(fg_color_u128).expect("infallible").into();
        let focused_bg_color: Color = u32::try_from(focused_bg_color_u128)
            .expect("infallible")
            .into();
        let focused_fg_color: Color = u32::try_from(focused_fg_color_u128)
            .expect("infallible")
            .into();
        let selector_color: Color = u32::try_from(selector_color_u128)
            .expect("infallible")
            .into();

        Ok(Self {
            element_id,
            viewport_id,
            row,
            col,
            bg_color,
            fg_color,
            selector_color,
            focused_bg_color,
            focused_fg_color,
            options,
            selector: 0,
        })
    }

    /// Store back to a Clarity value.
    /// Only returns the actionable data.
    fn to_clarity_value(&self) -> Result<Option<Value>, Error> {
        let value = Value::cons_list(
            self.options
                .clone()
                .into_iter()
                .map(|val| val.to_clarity_value())
                .collect(),
            &DEFAULT_WRB_EPOCH,
        )
        .expect("FATAL: failed to encode checkbox options list");
        Ok(Some(value))
    }

    /// Render the button
    fn render(&mut self, root: &mut Root, cursor: (u64, u64)) -> Result<(u64, u64), Error> {
        let focused = root.is_focused(self.element_id);
        let Some(viewport) = root.viewport_mut(self.viewport_id) else {
            return Err(Error::NoViewport(self.viewport_id));
        };
        wrb_test_debug!("Checkbox at ({},{})", self.row, self.col);

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

        let mut next_cursor = cursor;
        for (i, option) in self.options.iter().enumerate() {
            let row = self.row + u64::try_from(i).expect("infallible: too many options");
            next_cursor = if i == self.selector {
                viewport.print_to(
                    self.element_id,
                    row,
                    self.col,
                    self.selector_color,
                    fg_color,
                    &option.to_string(),
                )
            } else {
                viewport.print_to(
                    self.element_id,
                    row,
                    self.col,
                    bg_color,
                    fg_color,
                    &option.to_string(),
                )
            };
        }

        if focused {
            // set the cursor to be the checkbox at the selector
            root.set_form_cursor(
                self.viewport_id,
                self.row + u64::try_from(self.selector).unwrap_or(0),
                self.col + 1,
            );
        }
        Ok(next_cursor)
    }

    /// Handle an event
    fn handle_event(
        &mut self,
        root: &mut Root,
        event: WrbFormEvent,
    ) -> Result<Option<Value>, Error> {
        self.focus(root, root.is_focused(self.element_id))?;
        let WrbFormEvent::Keypress(keycode) = event else {
            return Ok(None);
        };

        // up and down move the selector
        if keycode == root.keycode_up() {
            self.selector = self.selector.saturating_sub(1);
            self.focus(root, root.is_focused(self.element_id))?;
            return Ok(None);
        }

        if keycode == root.keycode_down() {
            self.selector = self
                .selector
                .saturating_add(1)
                .min(self.options.len().saturating_sub(1));
            self.focus(root, root.is_focused(self.element_id))?;
            return Ok(None);
        }

        if keycode == root.keycode_enter() || keycode == root.keycode_space() {
            self.options[self.selector].selected = !self.options[self.selector].selected;
        }

        return Ok(None);
    }
}
