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

pub mod button;
pub mod checkbox;
pub mod print_text;
pub mod raw_text;
pub mod textline;
pub mod textarea;

#[cfg(test)]
pub mod tests;

use std::fmt;

pub use button::Button;
pub use checkbox::Checkbox;
pub use print_text::PrintText;
pub use raw_text::RawText;
pub use textline::TextLine;
pub use textarea::TextArea;

use termion::event::Key;

use crate::ui::root::Root;
use crate::ui::Error;

use crate::vm::storage::WritableWrbStore;

use clarity::vm::Value;

/// wrb UI type constants
/// These match the constants in `wrb.clar`
#[derive(Debug, Clone, PartialEq)]
pub enum WrbFormTypes {
    Text,
    Print,
    Button,
    Checkbox,
    TextLine,
    TextArea,
}

impl WrbFormTypes {
    pub fn as_u128(&self) -> u128 {
        match *self {
            Self::Text => 4,
            Self::Print => 5,
            Self::Button => 6,
            Self::Checkbox => 7,
            Self::TextLine => 8,
            Self::TextArea => 9,
        }
    }

    pub fn focusable(&self) -> bool {
        match *self {
            Self::Button
            | Self::Checkbox
            | Self::TextLine 
            | Self::TextArea => true,
            _ => false
        }
    }
}

impl TryFrom<u128> for WrbFormTypes {
    type Error = ();
    fn try_from(value: u128) -> Result<Self, Self::Error> {
        match value {
            4 => Ok(Self::Text),
            5 => Ok(Self::Print),
            6 => Ok(Self::Button),
            7 => Ok(Self::Checkbox),
            8 => Ok(Self::TextLine),
            9 => Ok(Self::TextArea),
            _ => Err(())
        }
    }
}

/// UI element event (specific to UI form elements, not to be confused with Wrb-wide events)
#[derive(Debug, Clone, PartialEq)]
pub enum WrbFormEvent {
    Keypress(Key),
}

/// Work around Clone blanket implementations not being object-safe
pub trait WrbFormClone {
    fn clone_box(&self) -> Box<dyn WrbForm>;
}

/// Work around Debug blanket implementations not being object-safe
pub trait WrbFormDebug {
    fn fmt_box(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
}

/// Work around PartialEq blanket implementations not being object-safe
pub trait WrbFormPartialEq {
    fn eq_box(&self, other: &Self) -> bool;
}

impl<T> WrbFormClone for T
where
    T: 'static + WrbForm + Clone,
{
    fn clone_box(&self) -> Box<dyn WrbForm> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn WrbForm> {
    fn clone(&self) -> Box<dyn WrbForm> {
        self.clone_box()
    }
}

impl<T> WrbFormDebug for T
where
    T: 'static + WrbForm + fmt::Debug,
{
    fn fmt_box(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt(f)
    }
}

impl fmt::Debug for Box<dyn WrbForm> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_box(f)
    }
}

/// Trait that each UI form element must implement 
pub trait WrbForm : Send + WrbFormClone + WrbFormDebug {
    /// What form ID are we?
    fn type_id(&self) -> WrbFormTypes;
    /// What is our element ID?
    fn element_id(&self) -> u128;
    /// What viewport are we attached to?
    fn viewport_id(&self) -> u128;
    /// Render this
    fn render(&mut self, root: &mut Root, cursor: (u64, u64)) -> Result<(u64, u64), Error>;
    /// Handle an (inputted) event.
    /// If an event is emitted, then it will immediately be fed into the wrbsite's event handler.
    fn handle_event(&mut self, root: &mut Root, event: WrbFormEvent) -> Result<Option<Value>, Error>;
    /// Deserialize the state from a Clarity value
    fn from_clarity_value(viewport_id: u128, value: Value) -> Result<Self, Error> where Self: Sized;
    /// Serialize the state to a Clarity value, so it can be stored to the wrbsite.
    /// If not applicable to this UI element, then the implementation should return None.
    fn to_clarity_value(&self) -> Result<Option<Value>, Error>;
}

