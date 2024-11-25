// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2022 Stacks Open Internet Foundation
// Copyright (C) 2022 Jude Nelson
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

use clarity::vm::errors::Error as clarity_error;
use std::collections::HashSet;
use std::convert::From;
use std::error;
use std::fmt;
use std::io;
use std::io::{Read, Write};

use crate::vm::storage::Error as DBError;
use crate::vm;

use clarity::vm::Value;
use clarity::vm::types::SequenceData;
use clarity::vm::types::CharType;
use clarity::vm::types::UTF8Data;
use clarity::vm::errors::InterpreterError;

use lzma_rs;

pub mod charbuff;
pub mod events;
pub mod forms;
pub mod render;
pub mod root;
pub mod scanline;
pub mod viewport;

pub use root::Root;
pub use root::SceneGraph;

#[cfg(test)]
pub mod tests;

pub use crate::ui::render::Renderer;

#[derive(Debug)]
pub enum Error {
    /// Something failed to encode or decode
    Codec(String),
    /// Something got too big (expected, actual)
    Overflow(usize, usize),
    /// I/O error
    IOError(io::Error),
    /// WRB VM error
    VMError(vm::Error),
    /// Clarity VM error
    Clarity(clarity_error),
    /// Given viewport does not exist
    NoViewport(u128),
    /// DB Error
    DB(DBError),
    /// WRB application page error
    Page(String),
    /// Event error
    Event(String)
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Codec(ref s) => write!(f, "Codec({})", s),
            Error::Overflow(exp, rcv) => write!(f, "too many bytes: expected {}, got {}", exp, rcv),
            Error::IOError(ref ioe) => ioe.fmt(f),
            Error::VMError(ref ve) => ve.fmt(f),
            Error::Clarity(ref ce) => ce.fmt(f),
            Error::NoViewport(ref idx) => write!(f, "No such viewport with ID {}", idx),
            Error::DB(ref dbe) => dbe.fmt(f),
            Error::Page(ref msg) => write!(f, "{}", msg),
            Error::Event(ref msg) => write!(f, "{}", msg),
        }
    }
}

impl error::Error for Error {
    fn cause(&self) -> Option<&dyn error::Error> {
        match *self {
            Error::Codec(_) => None,
            Error::Overflow(..) => None,
            Error::IOError(ref ioe) => Some(ioe),
            Error::VMError(ref ve) => Some(ve),
            Error::Clarity(ref ce) => Some(ce),
            Error::NoViewport(..) => None,
            Error::DB(ref dbe) => Some(dbe),
            Error::Page(..) => None,
            Error::Event(..) => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::IOError(e)
    }
}

impl From<lzma_rs::error::Error> for Error {
    fn from(e: lzma_rs::error::Error) -> Error {
        Error::Codec(format!("{:?}", &e))
    }
}

impl From<clarity_error> for Error {
    fn from(e: clarity_error) -> Error {
        Error::Clarity(e)
    }
}

impl From<DBError> for Error {
    fn from(e: DBError) -> Error {
        Error::DB(e)
    }
}

impl From<vm::Error> for Error {
    fn from(e: vm::Error) -> Error {
        Error::VMError(e)
    }
}

pub trait ValueExtensions {
    fn expect_utf8(self) -> Result<String, clarity_error>;
}

impl ValueExtensions for Value {
    fn expect_utf8(self) -> Result<String, clarity_error> {
        if let Value::Sequence(SequenceData::String(CharType::UTF8(UTF8Data { data }))) = self {
            let mut s = String::new();
            // each item in data is a code point
            for val_bytes in data.into_iter() {
                let val_4_bytes : [u8; 4] = match val_bytes.len() {
                    0 => [0, 0, 0, 0],
                    1 => [0, 0, 0, val_bytes[0]],
                    2 => [0, 0, val_bytes[0], val_bytes[1]],
                    3 => [0, val_bytes[0], val_bytes[1], val_bytes[2]],
                    4 => [val_bytes[0], val_bytes[1], val_bytes[2], val_bytes[3]],
                    _ => {
                        // invalid
                        s.push_str(&char::REPLACEMENT_CHARACTER.to_string());
                        continue;
                    }
                };
                let val_u32 = u32::from_be_bytes(val_4_bytes);
                let c = char::from_u32(val_u32).unwrap_or(char::REPLACEMENT_CHARACTER);
                s.push_str(&c.to_string());
            }
            Ok(s)
        } else {
            Err(clarity_error::Interpreter(InterpreterError::Expect("expected utf8 string".into())).into())
        }
    }
}
