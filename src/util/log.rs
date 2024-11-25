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

use std::fs;
use std::io;
use std::io::Write;
use std::fmt;
use std::env;
use std::sync::Mutex;

use crate::core::globals::with_logfile;

use stacks_common::util::get_epoch_time_ms;

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum WrbLogLevel {
    Debug = 3,
    Info = 2,
    Warn = 1,
    Error = 0,
}

impl WrbLogLevel {
    pub fn as_u8(&self) -> u8 {
        match *self {
            Self::Debug => 3,
            Self::Info => 2,
            Self::Warn => 1,
            Self::Error => 0
        }
    }
}

lazy_static! {
    pub static ref LOGLEVEL: Mutex<Option<WrbLogLevel>> = Mutex::new(None);
}

impl fmt::Display for WrbLogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            Self::Debug => write!(f, "DEBG"),
            Self::Info => write!(f, "INFO"),
            Self::Warn => write!(f, "WARN"),
            Self::Error => write!(f, "ERRO")
        }
    }
}

pub fn get_loglevel() -> WrbLogLevel {
    match LOGLEVEL.lock() {
        Ok(mut ll_opt) => {
            if let Some(ll) = ll_opt.as_ref() {
                *ll
            }
            else {
                if env::var("WRB_DEBUG") == Ok("1".into()) {
                    (*ll_opt).replace(WrbLogLevel::Debug);
                    WrbLogLevel::Debug
                }
                else {
                    (*ll_opt).replace(WrbLogLevel::Info);
                    WrbLogLevel::Info
                }
            }
        }
        Err(_e) => {
            panic!("FATAL: log mutex poisoned");
        }
    }
}

pub fn write_to_log(msg: &str) -> Result<(), io::Error> {
    with_logfile(|lf| {
        lf.write_all(msg.as_bytes())
    })
    .unwrap_or(Ok(()))
}

pub fn log_fmt(level: WrbLogLevel, file: &str, line: u32, msg: &str) -> String {
    let now = get_epoch_time_ms();
    format!("{} [{}.{:03}] [{}:{}] [{:?}]: {}\n", &level, now / 1000, now % 1000, file, line, std::thread::current().id(), msg)
}

#[macro_export]
macro_rules! wrb_test_debug {
    ($($arg:tt)*) => ({
        if cfg!(test) && crate::util::log::get_loglevel().as_u8() >= crate::util::log::WrbLogLevel::Debug.as_u8() {
            let _ = crate::util::log::write_to_log(&crate::util::log::log_fmt(crate::util::log::WrbLogLevel::Debug, file!(), line!(), &format!($($arg)*)));
        }
    })
}

#[macro_export]
macro_rules! wrb_debug {
    ($($arg:tt)*) => ({
        if crate::util::log::get_loglevel().as_u8() >= crate::util::log::WrbLogLevel::Debug.as_u8() {
            let _ = crate::util::log::write_to_log(&crate::util::log::log_fmt(crate::util::log::WrbLogLevel::Debug, file!(), line!(), &format!($($arg)*)));
        }
    })
}

#[macro_export]
macro_rules! wrb_info {
    ($($arg:tt)*) => ({
        if crate::util::log::get_loglevel().as_u8() >= crate::util::log::WrbLogLevel::Info.as_u8() {
            let _ = crate::util::log::write_to_log(&crate::util::log::log_fmt(crate::util::log::WrbLogLevel::Info, file!(), line!(), &format!($($arg)*)));
        }
    })
}

#[macro_export]
macro_rules! wrb_warn {
    ($($arg:tt)*) => ({
        if crate::util::log::get_loglevel().as_u8() >= crate::util::log::WrbLogLevel::Warn.as_u8() {
            let _ = crate::util::log::write_to_log(&crate::util::log::log_fmt(crate::util::log::WrbLogLevel::Warn, file!(), line!(), &format!($($arg)*)));
        }
    })
}

#[macro_export]
macro_rules! wrb_error {
    ($($arg:tt)*) => ({
        if crate::util::log::get_loglevel().as_u8() >= crate::util::log::WrbLogLevel::Error.as_u8() {
            let _ = crate::util::log::write_to_log(&crate::util::log::log_fmt(crate::util::log::WrbLogLevel::Error, file!(), line!(), &format!($($arg)*)));
        }
    })
}
