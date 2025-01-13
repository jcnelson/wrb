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

use termion::event::Key;

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::ui::charbuff::CharBuff;
use crate::ui::charbuff::CharCell;
use crate::ui::forms::WrbForm;
use crate::ui::forms::WrbFormEvent;
use crate::ui::viewport::Viewport;
use crate::ui::Error;

use clarity::vm::Value;

#[derive(Debug, Clone)]
pub enum ZBuffEntry {
    Root,
    Viewport(u128),
}

#[derive(Debug, Clone)]
pub struct SceneGraph {
    /// all viewports
    viewports: Vec<Viewport>,
    /// viewport table -- map viewport ID to index in self.viewports
    viewport_table: HashMap<u128, usize>,
    /// viewport-to-parent-index map
    tree: HashMap<u128, Option<usize>>,
    /// viewport hierarchy -- viewports at index i+1 have parents at index i
    hierarchy: Vec<Vec<u128>>,
    /// viewport absolute coordinate offsets.
    /// This + viewport.pos() = root coords
    coord_offsets: HashMap<u128, (u64, u64)>,
}

impl SceneGraph {
    pub fn new(viewports: Vec<Viewport>) -> SceneGraph {
        let mut viewport_table = HashMap::new();
        for (i, vp) in viewports.iter().enumerate() {
            viewport_table.insert(vp.id, i);
        }

        let mut tree = HashMap::new();
        for vp in viewports.iter() {
            let Some(parent_id) = vp.parent else {
                // root is the parent
                tree.insert(vp.id, None);
                continue;
            };
            let Some(i) = viewport_table.get(&parent_id) else {
                unreachable!("BUG: parent viewport has no entry in index table");
            };
            tree.insert(vp.id, Some(*i));
        }

        wrb_test_debug!("tree = {:?}", &tree);

        let mut coord_offsets = HashMap::new();
        let mut depths = BTreeMap::new();
        let mut max_depth = 0;
        for (i, vp) in viewports.iter().enumerate() {
            // walk vp to root
            let (mut row, mut col) = vp.pos();
            let mut id = vp.id;
            let mut depth = 0;
            while let Some(Some(parent_index)) = tree.get(&id).as_ref() {
                let parent = viewports
                    .get(*parent_index)
                    .expect("BUG: incorrectly-constructed scenegraph tree");
                if let Some((offset_row, offset_col)) = coord_offsets.get(&parent.id) {
                    // already processed this parent, so we can process this child
                    wrb_test_debug!(
                        "parent of viewport {} is processed -- at offset ({},{})",
                        id,
                        offset_row,
                        offset_col
                    );
                    row += offset_row;
                    col += offset_col;
                    let parent_depth = depths
                        .get(parent_index)
                        .expect("BUG: parent has cursor but no depth");
                    depth = parent_depth + 1;
                    break;
                }

                // have not processed the parent.
                let (parent_row, parent_col) = parent.pos();
                wrb_test_debug!(
                    "parent of viewport {} is NOT processed -- at offset ({},{})",
                    id,
                    parent_row,
                    parent_col
                );
                row += parent_row;
                col += parent_col;
                id = parent.id;
                depth += 1;
            }

            coord_offsets.insert(vp.id, (row, col));
            depths.insert(i, depth);
            max_depth = max_depth.max(depth);
        }

        wrb_test_debug!("depths = {:?}", &depths);
        wrb_test_debug!("coord_offsets = {:?}", &coord_offsets);

        let mut hierarchy = vec![vec![]; max_depth + 1];
        for (vp_idx, depth) in depths.into_iter() {
            let vp = viewports
                .get(vp_idx)
                .expect("BUG: incorrectly-constructed viewport table");
            hierarchy[depth].push(vp.id);
        }

        wrb_test_debug!("hierarchy = {:?}", &hierarchy);

        SceneGraph {
            viewports,
            viewport_table,
            tree,
            coord_offsets,
            hierarchy,
        }
    }

    /// Translate the viewport-relative (row,col) cursor into the root-level absolute (row,col)
    /// cursor
    pub fn abs_coords(&self, viewport_id: u128, rel_row: u64, rel_col: u64) -> Option<(u64, u64)> {
        self.coord_offsets
            .get(&viewport_id)
            .map(|(offset_row, offset_col)| (offset_row + rel_row, offset_col + rel_col))
    }

    /// Determine the viewport ID visible at the given absolute (row,col)
    pub fn viewport_at(&self, row: u64, col: u64, check_visible: bool) -> Option<u128> {
        for viewport_ids in self.hierarchy.iter().rev() {
            for viewport_id in viewport_ids.iter().rev() {
                let Some(vp_index) = self.viewport_table.get(viewport_id) else {
                    continue;
                };
                let Some(vp) = self.viewports.get(*vp_index) else {
                    continue;
                };
                if check_visible && !vp.visible() {
                    continue;
                }

                let Some((offset_row, offset_col)) = self.coord_offsets.get(viewport_id) else {
                    continue;
                };
                let (vp_rows, vp_cols) = vp.dims();

                if *offset_row <= row
                    && row < *offset_row + vp_rows
                    && *offset_col <= col
                    && col < *offset_col + vp_cols
                {
                    return Some(*viewport_id);
                }
            }
        }
        None
    }

    /// ref a viewport
    pub fn ref_viewport(&self, viewport_id: u128) -> Option<&Viewport> {
        let Some(vp_index) = self.viewport_table.get(&viewport_id) else {
            return None;
        };
        self.viewports.get(*vp_index)
    }

    /// ref viewports
    pub fn viewports(&self) -> &[Viewport] {
        &self.viewports
    }

    /// get viewport absolute coordinates on root pane
    pub fn viewport_coords(&self, viewport_id: u128) -> Option<(u64, u64)> {
        self.abs_coords(viewport_id, 0, 0)
    }
}

/// Root pane
#[derive(Clone, Debug)]
pub struct Root {
    /// All viewports
    scenegraph: SceneGraph,
    /// dimensions
    num_rows: u64,
    num_cols: u64,
    /// Z-buffer (indexes into viewports)
    zbuff: Option<Vec<ZBuffEntry>>,
    /// Delay betwen publishing this frame and polling the next, if desired (in milliseconds)
    pub frame_delay: Option<u64>,
    /// Which UI element (if any) has focus
    pub focused: Option<u128>,
    /// focus order
    pub(crate) focus_order: HashMap<u128, u128>,
    /// first element to focus
    pub(crate) focus_first: Option<u128>,
    /// form elements, keyed by element ID
    pub forms: HashMap<u128, Box<dyn WrbForm>>,
    /// where the cursor ought to be, if visible
    /// NOTE: this is 0-based, and is row,col
    pub cursor: Option<(u64, u64)>,
    /// which forms are dynamic, and can be replaced
    dynamic_forms: HashSet<u128>,
}

/// An update to the root pane
#[derive(Clone, Debug)]
pub struct FrameUpdate {
    pub new_contents: Vec<Box<dyn WrbForm>>,
}

impl Root {
    pub fn new(num_rows: u64, num_cols: u64, scenegraph: SceneGraph) -> Self {
        Self {
            scenegraph,
            num_rows,
            num_cols,
            zbuff: None,
            frame_delay: None,
            focused: None,
            focus_order: HashMap::new(),
            focus_first: None,
            forms: HashMap::new(),
            cursor: None,
            dynamic_forms: HashSet::new(),
        }
    }

    pub fn null() -> Self {
        Self::new(0, 0, SceneGraph::new(vec![]))
    }

    /// Calculate the z-buffer for the viewports.
    /// Viewports are considered ordered back-to-front.
    /// There are guaranteed to be `self.num_rows * self.num_cols` items in the zbuff
    pub(crate) fn make_zbuff(&self) -> Vec<ZBuffEntry> {
        let mut zbuff = Vec::with_capacity((self.num_rows * self.num_cols) as usize);
        for r in 0..self.num_rows {
            for c in 0..self.num_cols {
                let zbuff_entry = if let Some(viewport_id) = self.scenegraph.viewport_at(r, c, true)
                {
                    ZBuffEntry::Viewport(viewport_id)
                } else {
                    ZBuffEntry::Root
                };
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
                ZBuffEntry::Viewport(id) => output.push_str(&format!("{}", id)),
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
            let row = iu64 / self.num_cols;
            let col = iu64 % self.num_cols;
            if let ZBuffEntry::Viewport(viewport_id) = vpe {
                let viewport = self
                    .scenegraph
                    .ref_viewport(*viewport_id)
                    .expect("FATAL: zbuff points to a non-existent viewport");

                let (viewport_row, viewport_col) = self
                    .scenegraph
                    .viewport_coords(*viewport_id)
                    .expect("FATAL: zbuff points to a non-existent viewport");

                let (pos_row, pos_col) = viewport.pos();
                let charcell = viewport
                    .translate_coordinate(
                        viewport_row.saturating_sub(pos_row),
                        viewport_col.saturating_sub(pos_col),
                        row,
                        col,
                    )
                    .map(|(rel_row, rel_col)| viewport.charcell_at(rel_row, rel_col))
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

    /// Set and compute all forms
    pub fn set_all_forms(
        &mut self,
        static_ui_contents: Vec<Box<dyn WrbForm>>,
        dynamic_ui_contents: Vec<Box<dyn WrbForm>>,
    ) -> Result<(), Error> {
        let mut viewport_cursors = HashMap::new();
        let mut forms = HashMap::new();
        for mut ui_content in static_ui_contents.into_iter() {
            let viewport_id = ui_content.viewport_id();
            let cursor = viewport_cursors
                .get(&viewport_id)
                .cloned()
                .unwrap_or((0, 0));

            wrb_debug!("Create and render static form {}", ui_content.element_id());
            let new_cursor = ui_content.render(self, cursor)?;
            viewport_cursors.insert(viewport_id, new_cursor);
            forms.insert(ui_content.element_id(), ui_content);
        }
        let mut dynamic_form_ids = HashSet::new();
        for mut ui_content in dynamic_ui_contents.into_iter() {
            let viewport_id = ui_content.viewport_id();
            let cursor = viewport_cursors
                .get(&viewport_id)
                .cloned()
                .unwrap_or((0, 0));

            wrb_debug!("Create and render dynamic form {}", ui_content.element_id());
            let new_cursor = ui_content.render(self, cursor)?;
            viewport_cursors.insert(viewport_id, new_cursor);
            dynamic_form_ids.insert(ui_content.element_id());
            forms.insert(ui_content.element_id(), ui_content);
        }
        self.forms = forms;
        self.dynamic_forms = dynamic_form_ids;
        Ok(())
    }

    /// Redraw all forms
    pub fn redraw(&mut self) -> Result<(), Error> {
        let mut viewport_cursors = HashMap::new();
        let mut forms = std::mem::replace(&mut self.forms, HashMap::new());

        // redraw all forms and re-compute their cursors
        for (_element_id, ui_content) in forms.iter_mut() {
            let viewport_id = ui_content.viewport_id();
            let cursor = viewport_cursors.remove(&viewport_id).unwrap_or((0, 0));

            wrb_debug!("Redraw form {}", _element_id);
            let new_cursor = ui_content.render(self, cursor)?;
            viewport_cursors.insert(viewport_id, new_cursor);
        }

        self.forms = forms;
        Ok(())
    }

    /// Update from a FrameUpdate
    pub fn update_forms(&mut self, frame_update: FrameUpdate) -> Result<(), Error> {
        let mut viewport_cursors = HashMap::new();
        let mut forms = std::mem::replace(&mut self.forms, HashMap::new());

        // remove old forms
        let old_forms = std::mem::replace(&mut self.dynamic_forms, HashSet::new());
        for old_form_id in old_forms.into_iter() {
            forms.remove(&old_form_id);
        }

        // redraw static forms and re-compute their cursors
        for (_element_id, ui_content) in forms.iter_mut() {
            let viewport_id = ui_content.viewport_id();
            let cursor = viewport_cursors
                .get(&viewport_id)
                .cloned()
                .unwrap_or((0, 0));

            wrb_debug!("Render existing form {}", _element_id);
            let new_cursor = ui_content.render(self, cursor)?;
            viewport_cursors.insert(viewport_id, new_cursor);
        }

        // draw and add new forms
        let mut dynamic_form_ids = HashSet::new();
        for mut ui_content in frame_update.new_contents.into_iter() {
            let element_id = ui_content.element_id();
            let viewport_id = ui_content.viewport_id();
            let cursor = viewport_cursors
                .get(&viewport_id)
                .cloned()
                .unwrap_or((0, 0));

            wrb_debug!("Render new form {}", element_id);
            let new_cursor = ui_content.render(self, cursor)?;
            viewport_cursors.insert(viewport_id, new_cursor);
            forms.insert(element_id, ui_content);
            dynamic_form_ids.insert(element_id);
        }

        // clear focused if it references a nonexistant form
        let focused = if let Some(focused) = self.focused.take() {
            if forms.get(&focused).is_none() {
                None
            } else {
                Some(focused)
            }
        } else {
            None
        };

        self.forms = forms;
        self.dynamic_forms = dynamic_form_ids;
        self.focused = focused;
        Ok(())
    }

    pub fn render(&mut self) -> CharBuff {
        let buff = self.make_charbuff();
        self.make_focus_order(&buff);
        buff
    }

    /// Refresh using a new zbuff
    pub fn refresh(&mut self) -> CharBuff {
        self.zbuff = None;
        let buff = self.make_charbuff();
        self.make_focus_order(&buff);
        buff
    }

    /// Get a mutable ref to a viewport, given its ID
    pub fn viewport_mut(&mut self, id: u128) -> Option<&mut Viewport> {
        let Some(idx) = self.scenegraph.viewport_table.get(&id) else {
            return None;
        };
        self.scenegraph.viewports.get_mut(*idx)
    }

    /// ref viewports
    pub fn viewports(&self) -> &[Viewport] {
        &self.scenegraph.viewports
    }

    /// Look up a viewport ID given a (row,col) coordinate (uses the zbuff).
    /// Returns None if the zbuff is not instantiated, or is off the end of the zbuff
    pub fn find_viewport_id(&self, row: u64, col: u64) -> Option<u128> {
        let Some(zbuff) = self.zbuff.as_ref() else {
            return None;
        };
        let idx = usize::try_from(row * self.num_cols + col).ok()?;
        let Some(pt) = zbuff.get(idx) else {
            return None;
        };
        match pt {
            ZBuffEntry::Viewport(id) => Some(*id),
            ZBuffEntry::Root => None,
        }
    }

    /// Find the order of UI elements to shift focus to
    fn make_focus_order(&mut self, buff: &CharBuff) {
        let mut cur_ui_element = None;
        let mut element_ids = vec![];
        for cell in buff.cells.iter() {
            let CharCell::Fill {
                value: _value,
                bg: _bg,
                fg: _fg,
                element_id,
            } = cell
            else {
                continue;
            };
            let Some(form) = self.forms.get(element_id) else {
                continue;
            };
            if !form.type_id().focusable() {
                continue;
            }
            if let Some(cur_ui_element) = cur_ui_element.as_mut() {
                if *cur_ui_element != *element_id {
                    element_ids.push(*element_id);
                    *cur_ui_element = *element_id;
                }
            } else {
                element_ids.push(*element_id);
                cur_ui_element = Some(*element_id);
            }
        }

        if element_ids.len() == 0 {
            self.focus_order.clear();
            self.focus_first = None;
            return;
        }

        let mut focus_order = HashMap::new();
        for i in 0..(element_ids.len() - 1) {
            focus_order.insert(element_ids[i], element_ids[i + 1]);
        }
        focus_order.insert(element_ids[element_ids.len() - 1], element_ids[0]);
        self.focus_order = focus_order;
        self.focus_first = Some(element_ids[0]);
    }

    /// Update the focus pointer
    pub fn next_focus(&mut self) -> Result<(), Error> {
        let old_focused = self.focused.clone();
        if let Some(focused) = self.focused {
            let next_focused = self.focus_order.get(&focused).cloned();
            self.focused = next_focused;
        } else {
            self.focused = self.focus_first.clone();
        }
        if let Some(old_focused) = old_focused {
            if let Some(mut form) = self.forms.remove(&old_focused) {
                form.focus(self, false)?;
                self.forms.insert(old_focused, form);
            }
        }
        if let Some(focused) = self.focused {
            if let Some(mut form) = self.forms.remove(&focused) {
                form.focus(self, true)?;
                self.forms.insert(focused, form);
            }
        }
        Ok(())
    }

    pub fn clear_focus(&mut self) -> Result<(), Error> {
        if let Some(old_focused) = self.focused.take() {
            if let Some(mut form) = self.forms.remove(&old_focused) {
                form.focus(self, false)?;
                self.forms.insert(old_focused, form);
            }
        }
        Ok(())
    }

    /// Is this element focused?
    pub fn is_focused(&self, element_id: u128) -> bool {
        self.focused == Some(element_id)
    }

    /// Is the element ID focusable
    pub fn is_focusable(&self, element_id: u128) -> bool {
        self.focus_order.contains_key(&element_id)
    }

    /// Handle a form event. Pass it to the focused form.
    pub fn handle_event(&mut self, event: WrbFormEvent) -> Result<Option<Value>, Error> {
        let Some(focused) = self.focused else {
            wrb_debug!("No form focused; dropping event {:?}", &event);
            return Ok(None);
        };

        // take ownership to avoid multiple mutable references
        let Some(mut form) = self.forms.remove(&focused) else {
            wrb_debug!("No such form {}; dropping event {:?}", focused, &event);
            return Ok(None);
        };

        wrb_debug!("Pass event to form {}: {:?}", focused, &event);
        let res = form.handle_event(self, event);

        // restore form ownership to root
        self.forms.insert(focused, form);
        res
    }

    /// resolve a row/column within a form to the absolute row/column
    fn form_cursor_to_root_cursor(
        &self,
        form_viewport_id: u128,
        form_cursor_row: u64,
        form_cursor_col: u64,
    ) -> Option<(u64, u64)> {
        let Some((viewport_row, viewport_col)) = self.scenegraph.viewport_coords(form_viewport_id)
        else {
            return None;
        };

        Some((
            viewport_row + form_cursor_row,
            viewport_col + form_cursor_col,
        ))
    }

    /// Set the cursor, but relative to a form
    pub fn set_form_cursor(
        &mut self,
        form_element_id: u128,
        form_cursor_row: u64,
        form_cursor_col: u64,
    ) {
        let abs_coord =
            self.form_cursor_to_root_cursor(form_element_id, form_cursor_row, form_cursor_col);
        self.cursor = abs_coord;
    }

    /// What is the "enter" keycode?
    pub fn keycode_enter(&self) -> Key {
        Key::Char('\n')
    }

    /// What is the "space" keycode?
    pub fn keycode_space(&self) -> Key {
        Key::Char(' ')
    }

    /// What is the "up" keycode?
    pub fn keycode_up(&self) -> Key {
        Key::Up
    }

    /// What is the "down" keycode?
    pub fn keycode_down(&self) -> Key {
        Key::Down
    }
}
