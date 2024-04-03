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

use crate::ui::charbuff::CharBuff;
use crate::ui::charbuff::CharCell;
use crate::ui::charbuff::Color;

use termion::clear as termclear;
use termion::color as termcolor;
use termion::cursor as termcursor;

/// Rendering commands for the whole screen
#[derive(Clone, PartialEq, Debug)]
pub enum Scanline {
    /// set foreground color
    FgColor(Color),
    /// set background color
    BgColor(Color),
    /// A run of text
    Text(String),
    /// A newline
    Newline,
    /// Clear this line
    ClearLine,
    /// Reset the color
    ResetColor,
}

impl Scanline {
    /// Translate a charbuff into a sequence of scanline directives
    pub fn compile(buff: &CharBuff) -> Vec<Scanline> {
        let mut cmds = vec![];
        let mut cur_fg_color: Option<Color> = None;
        let mut cur_bg_color: Option<Color> = None;
        let mut cur_str: Vec<char> = vec![];
        let mut in_blank = true;

        fn finish_string(cur_str: &mut Vec<char>, cmds: &mut Vec<Scanline>) {
            if cur_str.len() > 0 {
                let s: String = cur_str.iter().map(|c| *c).collect();
                cmds.push(Scanline::Text(s));
                cur_str.clear();
            }
        }

        for (i, cell) in buff.cells.iter().enumerate() {
            if u64::try_from(i).expect("Infallible") % buff.num_cols == 0 {
                // finish up
                finish_string(&mut cur_str, &mut cmds);

                // next line
                if i > 0 {
                    cmds.push(Self::Newline);
                }
                cmds.push(Self::ClearLine);
                // carry over colors
                if let Some(fg) = cur_fg_color.as_ref() {
                    cmds.push(Self::FgColor(fg.clone()));
                }
                if let Some(bg) = cur_bg_color.as_ref() {
                    cmds.push(Self::BgColor(bg.clone()));
                }
            }
            match cell {
                CharCell::Blank => {
                    if !in_blank {
                        in_blank = true;
                        finish_string(&mut cur_str, &mut cmds);
                        cmds.push(Self::ResetColor);
                    }
                    cur_str.push(' ');
                    cur_fg_color = None;
                    cur_bg_color = None;
                }
                CharCell::Fill { fg, bg, value } => {
                    if in_blank {
                        finish_string(&mut cur_str, &mut cmds);
                        in_blank = false;
                    }

                    match cur_fg_color.take() {
                        Some(fg_color) => {
                            if fg != &fg_color {
                                finish_string(&mut cur_str, &mut cmds);
                                cmds.push(Self::FgColor(fg.clone()));
                            }
                            cur_fg_color = Some(fg.clone());
                        }
                        None => {
                            finish_string(&mut cur_str, &mut cmds);
                            cmds.push(Self::FgColor(fg.clone()));
                            cur_fg_color = Some(fg.clone());
                        }
                    }
                    match cur_bg_color.take() {
                        Some(bg_color) => {
                            if bg != &bg_color {
                                finish_string(&mut cur_str, &mut cmds);
                                cmds.push(Self::BgColor(bg.clone()));
                            }
                            cur_bg_color = Some(bg.clone());
                        }
                        None => {
                            finish_string(&mut cur_str, &mut cmds);
                            cmds.push(Self::BgColor(bg.clone()));
                            cur_bg_color = Some(bg.clone());
                        }
                    }

                    cur_str.push(*value);
                }
            }
        }
        finish_string(&mut cur_str, &mut cmds);
        cmds.push(Self::ResetColor);
        cmds
    }

    /// Translate a scanline command into its terminal control code string
    pub fn into_term_code(self) -> String {
        match self {
            Self::FgColor(color) => format!(
                "{}",
                termcolor::Fg(termcolor::Rgb(color.r, color.g, color.b))
            ),
            Self::BgColor(color) => format!(
                "{}",
                termcolor::Bg(termcolor::Rgb(color.r, color.g, color.b))
            ),
            Self::Text(s) => s,
            Self::Newline => "\r\n".into(),
            Self::ClearLine => format!("{}", termclear::CurrentLine),
            Self::ResetColor => format!(
                "{}{}",
                termcolor::Fg(termcolor::Reset),
                termcolor::Bg(termcolor::Reset)
            ),
        }
    }

    /// Translate a scanline command into just text
    pub fn into_text(self) -> String {
        match self {
            Self::FgColor(..)
            | Self::BgColor(..)
            | Self::ClearLine
            | Self::ResetColor => "".into(),
            Self::Text(s) => s,
            Self::Newline => "\n".into(),
        }
    }
}
