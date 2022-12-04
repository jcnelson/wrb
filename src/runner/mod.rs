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

use std::io;
use std::fmt;
use std::convert::TryFrom;
use std::error;

pub mod config;
pub mod process;

#[cfg(test)]
pub mod tests;

#[derive(Debug)]
pub enum Error {
    FailedToRun(String),
    FailedToExecute(String, io::Error),
    KilledBySignal(String),
    BadExit(i32),
    InvalidOutput(String)
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::FailedToRun(ref cmd) => write!(f, "Failed to run '{}'", cmd),
            Error::FailedToExecute(ref cmd, ref ioe) => write!(f, "Failed to run '{}': {:?}", cmd, ioe),
            Error::KilledBySignal(ref cmd) => write!(f, "Failed to run '{}': killed by signal", cmd),
            Error::BadExit(ref es) => write!(f, "Command exited with status {}", es),
            Error::InvalidOutput(ref s) => write!(f, "Invalid command output: '{}'", s)
        }
    }
}

impl error::Error for Error {
    fn cause(&self) -> Option<&dyn error::Error> {
        match *self {
            Error::FailedToRun(_) => None,
            Error::FailedToExecute(_, ref ioe) => Some(ioe),
            Error::KilledBySignal(_) => None,
            Error::BadExit(_) => None,
            Error::InvalidOutput(_) => None,
        }
    }
}

pub struct Config {
    /// Where the helper programs live
    helper_programs_dir: String,
    /// Where the node helper lives
    wrb_node_helper: String,
    /// where the gaia helper lives
    wrb_gaia_helper: String,
    /// where the wallet helper lives
    wrb_wallet_helper: String,
    /// where the sigin helper lives
    wrb_signin_helper: String,
    /// default node URL
    node_url: String,
    /// default Gaia hub URL
    gaia_url: String,
    /// mainnet or testnet
    mainnet: bool,
    /// maximum attachment size
    max_attachment_size: usize,
    /// number of columns
    num_columns: usize
}

pub struct Runner {
    config: Config
}

