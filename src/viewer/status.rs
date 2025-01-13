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

use termion::color;
use termion::event::Key;

use crate::ui::forms::TextLine;
use crate::ui::forms::WrbForm;
use crate::ui::forms::WrbFormEvent;
use crate::ui::Error;
use crate::ui::Root;

pub struct ViewerStatus {
    mode_text: String,
    at_top: bool,
    progress_text: TextLine,
}

impl ViewerStatus {
    pub fn new(wrb_name: String, at_top: bool) -> Self {
        Self {
            progress_text: TextLine::new_detached(wrb_name, 2048),
            mode_text: "(g)oto  |  (q)uit".into(),
            at_top,
        }
    }

    fn trunc_text(mut text: &str, num_cols: usize) -> String {
        text = match text.char_indices().nth(num_cols) {
            None => text,
            Some((idx, _)) => &text[0..idx],
        };

        text.to_string()
    }

    pub fn clear_text(&mut self) {
        self.progress_text.set_text("".to_string());
    }

    fn focus_prefix(&self) -> &'static str {
        "Goto: "
    }

    pub fn render(&self, focused: bool, num_cols: u64) -> String {
        let num_cols =
            usize::try_from(num_cols).expect("infallible -- num_cols doesn't fit a usize");
        let bg_color = if focused {
            color::Bg(color::Rgb(0xff, 0, 0xff))
        } else {
            color::Bg(color::Rgb(0xff, 0xff, 0))
        };

        let prefix = if focused { self.focus_prefix() } else { "" };

        let formatted_progress_text = format!(
            "{}{}{}{}{}",
            color::Fg(color::Black),
            bg_color,
            termion::clear::CurrentLine,
            prefix,
            Self::trunc_text(self.progress_text.text(), num_cols)
        );
        let formatted_mode_text = format!(
            "{}{}{}{}",
            color::Fg(color::White),
            color::Bg(color::Black),
            termion::clear::CurrentLine,
            Self::trunc_text(&self.mode_text, num_cols)
        );
        format!("{}\r\n{}", &formatted_progress_text, &formatted_mode_text)
    }

    pub fn num_rows(&self) -> u64 {
        2
    }

    pub fn at_top(&self) -> bool {
        self.at_top
    }

    pub fn handle_event(&mut self, event: WrbFormEvent) -> Result<(), Error> {
        self.progress_text.handle_event(&mut Root::null(), event)?;
        Ok(())
    }

    pub fn set_text(&mut self, txt: String) {
        self.progress_text.set_text(txt);
    }

    /// where should the cursor column be?
    pub fn cursor_column(&self, focused: bool) -> usize {
        self.progress_text.cursor()
            + if focused {
                self.focus_prefix().len()
            } else {
                0
            }
    }
}
