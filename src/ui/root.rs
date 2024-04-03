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

use std::collections::HashMap;

use crate::ui::charbuff::CharBuff;
use crate::ui::charbuff::CharCell;
use crate::ui::viewport::Viewport;

pub enum ZBuffEntry {
    Root,
    Viewport(usize),
}

/// Root pane
pub struct Root {
    /// All viewports
    viewports: Vec<Viewport>,
    /// map viewport ID to index in viewports
    viewport_table: HashMap<u128, usize>,
    /// dimensions
    num_rows: u64,
    num_cols: u64,
    /// Z-buffer (indexes into viewports)
    zbuff: Option<Vec<ZBuffEntry>>,
}

impl Root {
    fn make_viewport_table(viewports: &[Viewport]) -> HashMap<u128, usize> {
        let mut table = HashMap::new();
        for (i, vp) in viewports.iter().enumerate() {
            table.insert(vp.id, i);
        }
        table
    }

    pub fn new(num_cols: u64, num_rows: u64, viewports: Vec<Viewport>) -> Self {
        let viewport_table = Self::make_viewport_table(&viewports);
        Self {
            viewports,
            viewport_table,
            num_rows,
            num_cols,
            zbuff: None,
        }
    }

    /// Calculate the z-buffer for the viewports.
    /// Viewports are considered ordered back-to-front.
    /// There are guaranteed to be `self.num_rows * self.num_cols` items in the zbuff
    pub(crate) fn make_zbuff(&self) -> Vec<ZBuffEntry> {
        let mut zbuff = Vec::with_capacity((self.num_rows * self.num_cols) as usize);
        for r in 0..self.num_rows {
            for c in 0..self.num_cols {
                let mut zbuff_entry = ZBuffEntry::Root;
                for (i, viewport) in self.viewports.iter().enumerate() {
                    if viewport.translate_coordinate(c, r).is_none() {
                        continue;
                    }
                    if !viewport.visible() {
                        continue;
                    }
                    zbuff_entry = ZBuffEntry::Viewport(i);
                }
                zbuff.push(zbuff_entry);
            }
        }
        zbuff
    }

    #[cfg(test)]
    pub(crate) fn dump_zbuff(zbuff: &[ZBuffEntry], num_cols: u64) -> String {
        let mut output = String::new();
        for (i, zb) in zbuff.iter().enumerate() {
            let iu64 = i as u64;
            if iu64 > 0 && iu64 % num_cols == 0 {
                output.push_str("\n");
            }
            match zb {
                ZBuffEntry::Root => output.push_str("*"),
                ZBuffEntry::Viewport(i) => output.push_str(&format!("{}", i)),
            }
        }
        output
    }

    /// Calculate the frame to render as a charbuff
    fn make_charbuff(&mut self) -> CharBuff {
        let zbuff = self.zbuff.take().unwrap_or(self.make_zbuff());
        let mut buff = CharBuff::new(self.num_cols);
        for (i, vpe) in zbuff.iter().enumerate() {
            let iu64 = u64::try_from(i).expect("Infallible");
            let col = iu64 % self.num_cols;
            let row = iu64 / self.num_cols;
            if let ZBuffEntry::Viewport(vpi) = vpe {
                let viewport = self
                    .viewports
                    .get(*vpi)
                    .expect("FATAL: zbuff points to a non-existent viewport");
                let charcell = viewport
                    .translate_coordinate(col, row)
                    .map(|(rel_col, rel_row)| viewport.charcell_at(rel_col, rel_row))
                    .flatten()
                    .unwrap_or(CharCell::Blank);

                buff.cells.push(charcell);
            } else {
                // no viewport here
                buff.cells.push(CharCell::Blank);
            }
        }
        self.zbuff = Some(zbuff);
        buff
    }

    /// Render using the existing zbuff, or regenerating if it doesn't exist
    pub fn render(&mut self) -> CharBuff {
        self.make_charbuff()
    }

    /// Refresh using a new zbuff
    pub fn refresh(&mut self) -> CharBuff {
        self.zbuff = None;
        self.make_charbuff()
    }

    /// Get a mutable ref to a viewport, given its ID
    pub fn viewport_mut(&mut self, id: u128) -> Option<&mut Viewport> {
        let Some(idx) = self.viewport_table.get(&id) else {
            return None;
        };
        self.viewports.get_mut(*idx)
    }
}
