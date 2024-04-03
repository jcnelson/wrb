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

use std::fmt;

/// RGB color
#[derive(Clone, PartialEq, Debug, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl From<u32> for Color {
    fn from(c: u32) -> Self {
        Self {
            r: u8::try_from((c & 0x00ff0000) >> 16).expect("infallible"),
            g: u8::try_from((c & 0x0000ff00) >> 8).expect("infallible"),
            b: u8::try_from(c & 0x000000ff).expect("infallible"),
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// A single character cell in a character buffer
#[derive(Clone, PartialEq, Debug)]
pub enum CharCell {
    Blank,
    Fill { value: char, bg: Color, fg: Color },
}

impl CharCell {
    pub fn new(value: char, bg: Color, fg: Color) -> Self {
        Self::Fill { value, bg, fg }
    }
}

impl fmt::Display for CharCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blank => write!(f, "()()[]"),
            Self::Fill { value, bg, fg } => write!(f, "({})({})[{}]", &bg, &fg, value),
        }
    }
}

/// A buffer for holding characters that is a known number of columns wide
#[derive(Clone, PartialEq, Debug)]
pub struct CharBuff {
    pub num_cols: u64,
    pub cells: Vec<CharCell>,
}

impl CharBuff {
    pub fn new(num_cols: u64) -> Self {
        Self {
            num_cols,
            cells: vec![],
        }
    }

    pub fn clear(&mut self) {
        self.cells.clear();
    }

    pub fn num_rows(&self) -> u64 {
        (self.cells.len() as u64) / self.num_cols
    }

    /// Low-level print characters at a given starting column and row, with the given fg and bg
    /// colors.  Does not try to do word-wrapping.
    /// Returns the new (col, row) location where we last printed	
    pub fn print_at(&mut self, start_col: u64, start_row: u64, bg: Color, fg: Color, text: &str) -> (u64, u64) {
        // do we need to pad?
        let mut offset =
            usize::try_from(self.num_cols * start_row + start_col).expect("too much text");
        while self.cells.len() < offset {
            self.cells.push(CharCell::Blank);
        }

        for c in text.chars() {
            let ccell = if c <= '\x1f' || c.is_control() {
                // escape code or control character
                CharCell::new(char::REPLACEMENT_CHARACTER, bg, fg)
            } else {
                CharCell::new(c, bg, fg)
            };

            if offset < self.cells.len() {
                self.cells[offset] = ccell;
            } else {
                self.cells.push(ccell);
            }
            offset += 1;
        }
        let offset_u64 = u64::try_from(offset).expect("offset too big");
        (offset_u64 % self.num_cols, offset_u64 / self.num_cols)
    }

    /// Print word-wrapped text.
    /// Returns (end-col, end-row) where printing finished
    pub fn print(&mut self, start_col: u64, start_row: u64, bg: Color, fg: Color, text: &str) -> (u64, u64) {
        self.inner_print(start_col, start_row, bg, fg, text, false)
    }
    
    /// Print word-wrapped text with a newline at the end.
    /// Returns (end-col, end-row) where printing finished
    pub fn println(&mut self, start_col: u64, start_row: u64, bg: Color, fg: Color, text: &str) -> (u64, u64) {
        self.inner_print(start_col, start_row, bg, fg, text, true)
    }

    /// Print word-wrapped text, optionally with a terminating newline
    /// Returns (end-col, end-row) where printing finished
    fn inner_print(&mut self, start_col: u64, start_row: u64, bg: Color, fg: Color, text: &str, newline: bool) -> (u64, u64) {
        // split into words and spaces.
        let mut parts = vec![];
        let mut cur_part = String::new();
        let mut cur_len = 0;
        for c in text.chars() {
            if c.is_whitespace() {
                if cur_part.len() > 0 {
                    parts.push((cur_part.clone(), cur_len));
                    cur_part = String::new();
                    cur_len = 0;
                }
                parts.push((c.to_string(), 1));
            }
            else {
                cur_part.push_str(&c.to_string());
                cur_len += 1;
            }
        }
        // finish up
        if cur_part.len() > 0 {
            parts.push((cur_part.clone(), cur_len));
        }

        let mut idx = start_col;
        let mut row = start_row;
        let mut ret = (start_col, start_row);
        for (part, charlen) in parts {
            if idx + charlen < self.num_cols {
                // can write without wrap
                ret = self.print_at(idx, row, bg, fg, &part);
                idx += charlen;
            }
            else {
                // need to wrap
                row += 1;
                ret = self.print_at(0, row, bg, fg, &part);
                idx = charlen % self.num_cols;
            }

            // if part was too long to even fit into a row, then the word will have
            // automatically wrapped around. Update idx accordingly
            if charlen / self.num_cols >= 1 {
                row += charlen / self.num_cols;
            }
        }

        if newline {
            ret = (0, ret.1 + 1);
        }
        ret
    }

    /// Gets the charcell at the given (col, row) coordinate.
    /// Returns None if there's no cell allocated
    pub fn charcell_at(&self, col: u64, row: u64) -> Option<CharCell> {
        let idx = usize::try_from(self.num_cols * row + col).unwrap();
        self.cells.get(idx).cloned()
    }
}
