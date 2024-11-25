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

use crate::ui::forms::{WrbFormTypes, WrbFormEvent, WrbForm};

use termion::event::Key;

/// gap buffer for long text
#[derive(Debug, Clone, PartialEq)]
pub struct GapBuffer {
    /// message contents, plus a `gap`-sized range of 0's at `cursor`
    pub(crate) buffer: Vec<u32>,
    /// location of the gap
    pub(crate) cursor: usize,
    /// size the gap will be instantiated to
    pub(crate) gap: usize,
    /// current size of the gap
    pub(crate) gap_size: usize,
    /// which text line the cursor is in
    pub(crate) lineno: usize,
    /// offset into `buffer` where the line begins
    pub(crate) line_start: usize,
}

pub const GAP_SIZE : usize = 65536;

impl GapBuffer {
    pub fn new(start_text: &str, gap_size: usize) -> Self {
        let mut buffer = vec![0u32; start_text.len() + gap_size];

        // extract UTF-8 code points
        for (i, ch) in start_text.chars().enumerate() {
            buffer[i] = u32::from(ch);
        }
        let mut gapbuffer = Self {
            buffer,
            cursor: start_text.len(),
            gap: gap_size,
            gap_size,
            lineno: Self::count_lines(start_text),
            line_start: 0
        };
        gapbuffer.line_start = gapbuffer.find_line_start();
        gapbuffer
    }

    fn count_lines(txt: &str) -> usize {
        let mut line_count = 0;
        for c in txt.chars() {
            if c == '\n' {
                line_count += 1;
            }
        }
        line_count
    }

    /// Find the index into the gap buffer where the start of the line at `self.cursor` is
    fn find_line_start(&self) -> usize {
        let eof = self.chr().is_none();
        let mut i = self.cursor.saturating_sub(1);
        if self.buffer[i] == u32::from('\n') && eof {
            return self.cursor;
        }

        while i > 0 && self.buffer[i] != u32::from('\n') {
            i = i.saturating_sub(1);
        }

        if self.cursor > 0 && i.saturating_add(1) + self.gap < self.buffer.len() && self.buffer[i] == u32::from('\n') {
            i = i.saturating_add(1);
        }
        i
    }

    /// Find the index into the gap buffer where ithe start of the next line is, where the current
    /// line is identified by the line indexed by `start_idx`.
    fn find_next_line_start_at(&self, start_idx: usize) -> usize {
        let mut i = start_idx;
        if i + self.gap == self.buffer.len() {
            return i;
        }
        while i + self.gap < self.buffer.len() && self.buffer[i] != u32::from('\n') {
            i = i.saturating_add(1);
        }
        if self.buffer[i] == u32::from('\n') {
            i = i.saturating_add(1);
        }
        i
    }

    /// Find the index into the gap buffer where the Nth line after the line identified by
    /// `start_idx` is (where N is `num_lines`).
    fn find_next_lines_start_at(&self, start_idx: usize, num_lines: usize) -> usize {
        let mut i = start_idx;
        for _ in 0..num_lines {
            i = self.find_next_line_start_at(i);
        }
        i
    }

    fn realloc(&mut self) {
        // add new space at the end of the buffer
        let gap_start = self.buffer.len();
        self.buffer.resize(self.buffer.len() + self.gap_size, 0);
        self.gap = self.gap_size;

        // move the gap to trail the cursor.
        // move all text from the cursor to the start of the gap over to where the gap currently
        // is.
        for i in 0..self.gap_size {
            if self.cursor + i >= gap_start {
                break;
            }

            let ch = self.buffer[self.cursor + i];
            self.buffer[gap_start + self.cursor + i] = ch;
            self.buffer[self.cursor + i] = 0;
        }
    }

    /// insert a character at a position
    pub fn insert(&mut self, ch: char) {
        if self.gap == 0 {
            // re-alloc
            self.realloc();
        }
        assert_eq!(self.buffer[self.cursor], 0);
        self.buffer[self.cursor] = u32::from(ch);
        self.cursor += 1;
        self.gap -= 1;
        if ch == '\n' {
            self.lineno += 1;
            self.line_start = self.find_line_start();
        }
    }

    /// backspace a character at the cursor
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }

        let chr = self.buffer[self.cursor];
        self.cursor -= 1;
        self.buffer[self.cursor] = 0;
        self.gap += 1;

        if chr == u32::from('\n') {
            self.lineno = self.lineno.saturating_sub(1);
            self.line_start = self.find_line_start();
        }
    }
    
    /// delete a character at the cursor
    pub fn delete(&mut self) {
        self.right();
        self.backspace();
    }

    /// replace a character at the cursor
    pub fn replace(&mut self, ch: char) {
        self.delete();
        self.insert(ch);
        self.left();
    }

    /// move cursor left, and shift the gap with it.
    /// Return true if cursor moved; false if not.
    pub fn left(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        let ch = self.buffer[self.cursor];
        self.buffer[self.cursor] = 0;
        self.buffer[self.cursor + self.gap] = ch;

        if ch == u32::from('\n') {
            self.lineno += 1;
            self.line_start = self.find_line_start();
        }
        true
    }

    /// move cursor right, and shift the gap with it.
    /// Return true if cursor moved; false if not.
    pub fn right(&mut self) -> bool {
        if self.cursor + self.gap == self.buffer.len() {
            return false;
        }

        let ch = self.buffer[self.cursor + self.gap];
        self.buffer[self.cursor + self.gap] = 0;
        self.buffer[self.cursor] = ch;
        self.cursor += 1;
        
        if ch == u32::from('\n') {
            self.lineno = self.lineno.saturating_sub(1);
            self.line_start = self.find_line_start();
        }
        true
    }

    /// move cursor to the start of the current or last word
    /// cursor will be at the first character of the word
    pub fn left_word(&mut self) -> bool {
        let old_cursor = self.cursor;
        while self.chr().map(|c| c.is_whitespace()).unwrap_or(true) {
            if !self.left() {
                break;
            }
        }
        while !self.chr().map(|c| c.is_whitespace()).unwrap_or(true) {
            if !self.left() {
                break;
            }
        }
        self.right();
        self.cursor == old_cursor
    }

    /// move the cursor to the start of the next word
    pub fn right_word(&mut self) -> bool {
        let old_cursor = self.cursor;
        while self.chr().map(|c| c.is_whitespace()).unwrap_or(true) {
            if !self.right() {
                break;
            }
        }
        self.cursor == old_cursor
    }

    /// move the cursor to the start of the line
    /// return true if cursor moved; false if not
    pub fn line_start(&mut self) -> bool {
        let old_cursor = self.cursor;
        if self.chr().is_none() {
            // end of text
            if !self.left() {
                return false;
            }
        }
        if self.chr().map(|c| c == '\n').unwrap_or(true) {
            if !self.left() {
                return false;
            }
        }
        while self.chr().map(|c| c != '\n').unwrap_or(true) {
            if !self.left() {
                break;
            }
        }
        if self.chr().map(|c| c == '\n').unwrap_or(true) {
            if !self.right() {
                return false;
            }
        }
        self.cursor == old_cursor
    }

    /// move the cursor to the end of the line
    pub fn line_end(&mut self) -> bool {
        let old_cursor = self.cursor;
        while self.chr().map(|c| c != '\n').unwrap_or(true) {
            if !self.right() {
                break;
            }
        }
        self.cursor == old_cursor
    }
    
    /// shift the cursor up one row, and shift the gap with it.  Takes into account newlines.
    ///
    /// Put the cursor at a location such that it's either at the same column (relative to the
    /// given width), or barring that, as close to it from the left as possible.
    /// If we can't shift up -- i.e. we're in the first line, given the width, then do nothing
    pub fn up(&mut self, col: usize) {
        if self.line_start == 0 {
            // in the first line already
            return;
        }

        self.line_start();
        self.left();
        self.line_start = self.find_line_start();
        self.cursor = self.line_start;

        for _ in 0..col {
            if self.chr().map(|c| c != '\n').unwrap_or(true) {
                break;
            }
            self.right();
        }
    }

    /// shift the cursor down a row, and shift the gap with it.  Takes into account newlines
    ///
    /// Put the cursor at a location such that it's either at the same column (relative to the
    /// given width), or barring that, as close to it from the left as possible.
    /// If we can't shift down -- i.e. we're in the last line, given the width, then do nothing
    pub fn down(&mut self, col: usize) {
        self.line_end();
        self.right();
        self.line_start = self.find_line_start();
        for _ in 0..col {
            if self.chr().map(|c| c != '\n').unwrap_or(true) {
                break;
            }
            self.right();
        }
    }

    /// Set the inner text
    /// Resets all state
    pub fn set_text(&mut self, text: String) {
        let mut buffer = vec![0u32; text.len() + self.gap_size];

        // extract UTF-8 code points
        for (i, ch) in text.chars().enumerate() {
            buffer[i] = u32::from(ch);
        }

        self.cursor = buffer.len();
        self.buffer = buffer;
        self.gap = self.gap_size;
        self.lineno = Self::count_lines(text.as_str());
        self.line_start = self.find_line_start();
    }

    /// How long is the inner text?
    pub fn len(&self) -> usize {
        self.buffer.len().saturating_sub(self.gap)
    }

    pub fn get(&self, idx: usize) -> Option<char> {
        if idx < self.cursor {
            return self.buffer.get(idx).map(|x| char::from_u32(*x)).flatten();
        }
        else {
            return self.buffer.get(idx + self.gap).map(|x| char::from_u32(*x)).flatten();
        }
    }

    pub fn chr(&self) -> Option<char> {
        return self.buffer.get(self.cursor + self.gap).map(|x| char::from_u32(*x).unwrap_or(char::REPLACEMENT_CHARACTER))
    }

    pub fn line(&self) -> usize {
        self.lineno
    }

    /// Where are we?
    pub fn get_cursor(&self) -> usize {
        self.cursor
    }

    pub fn iter<'a>(&'a self) -> GapBufferIterator<'a> {
        GapBufferIterator {
            idx: 0,
            buff: self
        }
    }

    pub fn iter_at_offset<'a>(&'a self, offset: usize) -> GapBufferIterator<'a> {
        GapBufferIterator {
            idx: offset,
            buff: self
        }
    }

    pub fn to_string(&self) -> String {
        let mut ret = String::new();
        for c in self.iter() {
            ret.push(c);
        }
        ret
    }
    
    /// index into the gap buffer of the last character in the visible region is.
    pub fn end_of_area(&self, scroll: usize, num_rows: usize, num_cols: usize) -> usize {
        let mut row = 0;
        let mut col = 0;
        let mut i = scroll;
        if num_rows == 0 || num_cols == 0 {
            return None;
        }
        while row < num_rows {
            let Some(c) = self.get(i) else {
                break;
            }
            i += 1;

            if c == '\n' {
                row += 1;
            }
            if i > 0 && i % num_cols == 0 {
                row += 1;
            }
        }
        i
    }

    /// find the (row,col) location of the cursor in the text area, given the scroll offset index.
    /// Return None if not visible
    pub fn cursor_location(&self, scroll: usize, num_rows: usize, num_cols: usize) -> Option<(u64, u64)> {
        if self.cursor < scroll {
            return None;
        }
        if num_rows == 0 || num_cols == 0 {
            return None;
        }

        let mut i = scroll;
        let mut row = 0;
        let mut col = 0l
        while i < self.cursor {
            let Some(c) = self.get(i) else {
                break;
            }
            i += 1;
            col += 1;
            if c == '\n' {
                row += 1;
                col = 0;
            }
            if i > 0 && i % num_cols == 0 {
                row += 1;
                col = 0;
            }

            if row >= num_rows {
                // not visible
                return None;
            }
        }
        Some((row, col))
    }
}

pub struct GapBufferIterator<'a> {
    idx: usize,
    buff: &'a GapBuffer
}

impl<'a> Iterator for GapBufferIterator<'a> {
    type Item = char;
    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.buff.get(self.idx);
        self.idx += 1;
        ret
    }
}

/// UI command to add an editable line of text
#[derive(Clone, PartialEq, Debug)]
pub struct TextArea {
    element_id: u128,
    viewport_id: u128,
    row: u64,
    col: u64,
    num_rows: u64,
    num_cols: u64,
    bg_color: Color,
    fg_color: Color,
    focused_bg_color: Color,
    focused_fg_color: Color,
    inner_text: GapBuffer,
    max_len: usize,
    insert: bool,
    /// Index into the `inner_text` gap buffer where the first character to be displayed is.
    scroll: usize,
    /// desired column for the cursor when moving up or down
    cursor_col: usize,
}

impl TextArea {
    pub fn text(&self) -> String {
        self.inner_text.to_string()
    }

    pub fn set_text(&mut self, txt: String) {
        self.inner_text.set_text(txt);
    }

    pub fn cursor(&self) -> usize {
        self.inner_text.cursor
    }

    pub fn insert(&self) -> bool {
        self.insert
    }
}

impl WrbForm for TextArea {
    fn type_id(&self) -> WrbFormTypes {
        WrbFormTypes::TextArea
    }
    
    fn element_id(&self) -> u128 {
        self.element_id
    }
    
    fn viewport_id(&self) -> u128 {
        self.viewport_id
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
        
        let num_rows = text_tuple
            .get("num-rows")
            .cloned()
            .expect("FATAL: no `num-rows`")
            .expect_u128()?;
        
        let num_cols = text_tuple
            .get("num-cols")
            .cloned()
            .expect("FATAL: no `num-cols`")
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

        let bg_color : Color = u32::try_from(bg_color_u128).expect("infallible").into();
        let fg_color : Color = u32::try_from(fg_color_u128).expect("infallible").into();
        let focused_bg_color : Color = u32::try_from(focused_bg_color_u128).expect("infallible").into();
        let focused_fg_color : Color = u32::try_from(focused_fg_color_u128).expect("infallible").into();

        Ok(TextArea {
            element_id,
            viewport_id,
            row: u64::try_from(row).map_err(|_| Error::Codec("row too big".into()))?,
            col: u64::try_from(col).map_err(|_| Error::Codec("col too big".into()))?,
            num_rows: u64::try_from(num_rows).map_err(|_| Error::Codec("num-rows too big".into()))?,
            num_cols: u64::try_from(num_cols).map_err(|_| Error::Codec("num-cols too big".into()))?,
            max_len: usize::try_from(max_len).map_err(|_| Error::Codec("max-len too big".into()))?,
            bg_color,
            fg_color,
            focused_bg_color,
            focused_fg_color,
            inner_text: GapBuffer::new(&text, GAP_SIZE),
            insert: true,
            scroll: 0,
            cursor_col: 0,
        })
    }

    fn to_clarity_value(&self) -> Result<Option<Value>, Error> {
        // TODO
        Ok(None)
    }

    fn render(&mut self, root: &mut Root, _cursor: (u64, u64)) -> Result<(u64, u64), Error> {
        let focused = root.is_focused(self.element_id);
        let Some(viewport) = root.viewport_mut(self.viewport_id) else {
            return Err(Error::NoViewport(self.viewport_id));
        };
        let bg_color = if focused {
            self.focused_bg_color.clone()
        }
        else {
            self.bg_color.clone()
        };
        let fg_color = if focused {
            self.focused_fg_color.clone()
        }
        else {
            self.fg_color.clone()
        };

        let new_cursor = viewport.print_iter(self.element_id, self.row, self.col, bg_color, fg_color, self.inner_text.iter_at_offset(self.scroll));
        if focused {
            if let Some((cursor_row, cursor_column)) = self.inner_text.cursor_location(self.scroll, self.num_rows, self.num_cols) {
                root.set_form_cursor(
                    self.element_id,
                    self.row + cursor_row,
                    self.col + cursor_col
                );
            }
        }

        Ok(new_cursor)
    }
    
    /// This doesn't generate an event the main loop cares about, but it does update the text
    /// buffer.
    fn handle_event(&mut self, _root: &mut Root, event: WrbFormEvent) -> Result<Option<Value>, Error> {
        match event {
            WrbFormEvent::Keypress(key) => {
                match key {
                    Key::Left => {
                        self.inner_text.left();
                        self.cursor_col = self.inner_text.cursor.saturating_sub(self.inner_text.line_start);
                        if self.inner_text.cursor < self.scroll {
                            self.scroll = self.inner_text.find_line_start();
                        }
                    }
                    Key::Right => {
                        self.inner_text.right();
                        self.cursor_col = self.inner_text.cursor.saturating_sub(self.inner_text.line_start);
                        if self.inner_text.cursor >= self.inner_text.end_of_area(self.scroll, self.num_rows, self.num_cols) {
                            self.scroll = self.inner_text.find_line_start();
                        }
                    }
                    Key::Up => {
                        self.inner_text.up(self.cursor_col);
                        if self.inner_text.cursor < self.scroll {
                            self.scroll = self.inner_text.find_line_start();
                        }
                    }
                    Key::Down => {
                        self.inner_text.down(self.cursor_col);
                        if self.inner_text.cursor >= self.inner_text.end_of_area(self.scroll, self.num_rows, self.num_cols) {
                            self.scroll = self.inner_text.find_line_start();
                        }
                    }
                    Key::Backspace => {
                        self.inner_text.backspace();
                        if self.inner_text.cursor < self.scroll {
                            self.scroll = self.inner_text.find_line_start();
                    }
                    Key::Delete => {
                        self.inner_text.delete();
                        if self.inner_text.cursor < self.scroll {
                            self.scroll = self.inner_text.find_line_start();
                    }
                    Key::Insert => {
                        self.insert = !self.insert;
                    }
                    Key::Home => {
                        self.inner_text.line_start();
                        if self.inner_text.cursor < self.scroll {
                            self.scroll = self.inner_text.find_line_start();
                    }
                    Key::End => {
                        self.inner_text.line_end();
                        if self.inner_text.cursor >= self.inner_text.end_of_area(self.scroll, self.num_rows, self.num_cols) {
                            self.scroll = self.inner_text.find_line_start();
                        }
                    }
                    Key::Char(c) => {
                        if self.insert {
                            self.inner_text.insert(c);
                        }
                        else {
                            self.inner_text.replace(c);
                        }
                        if self.inner_text.cursor >= self.inner_text.end_of_area(self.scroll, self.num_rows, self.num_cols) {
                            self.scroll = self.inner_text.find_line_start();
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(None)
    }
}

