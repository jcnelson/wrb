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

use clarity::vm::Value;
use crate::ui::Error;
use crate::ui::charbuff::Color;
use crate::ui::root::Root;
use crate::ui::ValueExtensions;

use crate::ui::forms::WrbForm;
use crate::ui::forms::WrbFormTypes;
use crate::ui::forms::WrbFormEvent;

/// UI command to produce a button
#[derive(Clone, PartialEq, Debug)]
pub struct Button {
    element_id: u128,
    viewport_id: u128,
    row: u64,
    col: u64,
    bg_color: Color,
    fg_color: Color,
    focused_bg_color: Color,
    focused_fg_color: Color,
    text: String,
}

impl WrbForm for Button {
    /// type
    fn type_id(&self) -> WrbFormTypes {
        WrbFormTypes::Button
    }

    fn element_id(&self) -> u128 {
        self.element_id
    }
    
    fn viewport_id(&self) -> u128 {
        self.viewport_id
    }

    fn focus(&mut self, root: &mut Root, focused: bool) -> Result<(), Error> {
        if focused {
            root.set_form_cursor(self.viewport_id, self.row, self.col + 1);
        }
        Ok(())
    }
    
    /// Construct from Clarity value
    fn from_clarity_value(viewport_id: u128, v: Value) -> Result<Self, Error> {
        let button_tuple = v.expect_tuple()?;
        let text = button_tuple
            .get("text")
            .cloned()
            .expect("FATAL: no `text`")
            .expect_utf8()?;

        let row = u64::try_from(button_tuple
            .get("row")
            .cloned()
            .expect("FATAL: no `row`")
            .expect_u128()?)
            .map_err(|_| Error::Codec("row is too big".into()))?;
        
        let col = u64::try_from(button_tuple
            .get("col")
            .cloned()
            .expect("FATAL: no `col`")
            .expect_u128()?)
            .map_err(|_| Error::Codec("col is too big".into()))?;

        let bg_color_u128 = button_tuple
            .get("bg-color")
            .cloned()
            .expect("FATAL: no `bg-color`")
            .expect_u128()?
            // truncate
            & 0xffffffffu128;
        
        let fg_color_u128 = button_tuple
            .get("fg-color")
            .cloned()
            .expect("FATAL: no `fg-color`")
            .expect_u128()?
            // trunate
            &0xffffffffu128;
        
        let focused_bg_color_u128 = button_tuple
            .get("focused-bg-color")
            .cloned()
            .expect("FATAL: no `focused-bg-color`")
            .expect_u128()?
            // truncate
            & 0xffffffffu128;
        
        let focused_fg_color_u128 = button_tuple
            .get("focused-fg-color")
            .cloned()
            .expect("FATAL: no `focused-fg-color`")
            .expect_u128()?
            // trunate
            &0xffffffffu128;

        let element_id = button_tuple
            .get("element-id")
            .cloned()
            .expect("FATAL: no `element-id`")
            .expect_u128()?;

        let bg_color : Color = u32::try_from(bg_color_u128).expect("infallible").into();
        let fg_color : Color = u32::try_from(fg_color_u128).expect("infallible").into();
        let focused_bg_color : Color = u32::try_from(focused_bg_color_u128).expect("infallible").into();
        let focused_fg_color : Color = u32::try_from(focused_fg_color_u128).expect("infallible").into();

        Ok(Button {
            element_id,
            viewport_id,
            row,
            col,
            bg_color,
            fg_color,
            focused_bg_color,
            focused_fg_color,
            text,
        })
    }

    /// Convert to Clarity value
    /// Because Buttons don't have any mutable state to store, don't do anything
    fn to_clarity_value(&self) -> Result<Option<Value>, Error> {
        Ok(None)
    }

    /// Render the button
    fn render(&mut self, root: &mut Root, _cursor: (u64, u64)) -> Result<(u64, u64), Error> {
        let focused = root.is_focused(self.element_id);
        let Some(viewport) = root.viewport_mut(self.viewport_id) else {
            return Err(Error::NoViewport(self.viewport_id));
        };
        wrb_test_debug!("Button '{}' at ({},{})", &self.text, self.row, self.col);
        let new_cursor = if focused {
            viewport.print_to(self.element_id, self.row, self.col, self.focused_bg_color, self.focused_fg_color, &format!("<{}>", &self.text))
        }
        else {
            viewport.print_to(self.element_id, self.row, self.col, self.bg_color, self.fg_color, &format!("[{}]", &self.text))
        };

        if focused {
            // point the cursor at the start of the button text
            root.set_form_cursor(self.viewport_id, self.row, self.col + 1);
        }
        Ok(new_cursor)
    }
    
    /// Handle an event
    fn handle_event(&mut self, root: &mut Root, event: WrbFormEvent) -> Result<Option<Value>, Error> {
        self.focus(root, root.is_focused(self.element_id))?;
        let WrbFormEvent::Keypress(keycode) = event else {
            return Ok(None);
        };
        if keycode != root.keycode_enter() {
            // not the "submit" keycode
            return Ok(None);
        }

        // indicate that we've been pressed
        Ok(Some(Value::UInt(self.element_id)))
    }
}

