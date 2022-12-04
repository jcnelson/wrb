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

use std::fmt;
use std::io;
use std::collections::HashSet;
use std::convert::From;
use std::io::{Read, Write};
use std::error;
use clarity::vm::errors::Error as clarity_error;

use pulldown_cmark::Alignment as CMAlignment;
use lzma_rs;

pub mod render;

#[cfg(test)]
pub mod tests;

#[derive(Debug)]
pub enum Error {
    Codec(String),
    Overflow(usize, usize),
    IOError(io::Error),
    Clarity(clarity_error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Codec(ref s) => write!(f, "Codec({})", s),
            Error::Overflow(exp, rcv) => write!(f, "too many bytes: expected {}, got {}", exp, rcv),
            Error::IOError(ref ioe) => ioe.fmt(f),
            Error::Clarity(ref ce) => ce.fmt(f),
        }
    }
}

impl error::Error for Error {
    fn cause(&self) -> Option<&dyn error::Error> {
        match *self {
            Error::Codec(_) => None,
            Error::Overflow(..) => None,
            Error::IOError(ref ioe) => Some(ioe),
            Error::Clarity(ref ce) => Some(ce)
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

enum TableState {
    Header(Vec<CMAlignment>),
    Body,
}

pub struct Renderer {
    /// maximum attachment size -- a decoded string can't be longer than this
    max_attachment_size: usize,
    /// block quote level
    block_quote_level: usize,
    /// list stack.  items are list numbers
    list_stack: Vec<Option<u64>>,
    /// are we in a table, and if so, where are we?
    table_state: Option<TableState>,
    /// footnote labels
    footnote_labels: HashSet<String>,
}
