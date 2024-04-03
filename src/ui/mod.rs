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

use lzma_rs;

pub mod charbuff;
pub mod render;
pub mod root;
pub mod scanline;
pub mod viewport;

#[cfg(test)]
pub mod tests;

#[derive(Debug)]
pub enum Error {
    Codec(String),
    Overflow(usize, usize),
    IOError(io::Error),
    VMError(vm::Error),
    Clarity(clarity_error),
    NoViewport(u128),
    DB(DBError),
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

pub struct Renderer {
    /// maximum attachment size -- a decoded string can't be longer than this
    max_attachment_size: u64,
}
