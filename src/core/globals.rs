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

use std::collections::HashMap;
use std::sync::Mutex;

use clarity::vm::types::QualifiedContractIdentifier;

use crate::core::Config;

use crate::storage::StackerDBClient;
use crate::storage::Wrbpod;
use crate::storage::WrbpodAddress;

use std::fs::File;
use std::io;
use std::io::Write;

use rand::thread_rng;
use rand::Rng;
use rand::RngCore;

/// Globally-accessible state that is hard to pass around otherwise
pub struct Globals {
    pub config: Option<Config>,
    /// Maps session IDs to wrbpod state
    pub wrbpod_sessions: HashMap<u128, Wrbpod>,
    /// Maps contract IDs to session IDs
    pub wrbpod_addr_to_session_id: HashMap<WrbpodAddress, u128>,
    /// Next wrbpod session ID
    next_wrbpod_session_id: u128,
}

impl Default for Globals {
    fn default() -> Globals {
        Globals {
            config: None,
            wrbpod_sessions: HashMap::new(),
            wrbpod_addr_to_session_id: HashMap::new(),
            next_wrbpod_session_id: 0,
        }
    }
}

impl Globals {
    pub fn new() -> Globals {
        Globals::default()
    }

    pub fn reset(&mut self) {
        self.wrbpod_sessions.clear();
    }

    pub fn get_config(&self) -> Config {
        self.config.clone().expect("FATAL: config not initialized")
    }

    pub fn config_ref(&self) -> &Config {
        self.config.as_ref().expect("FATAL: config not initialized")
    }

    pub fn config_mut(&mut self) -> &mut Config {
        self.config.as_mut().expect("FATAL: config not initialized")
    }

    pub fn set_config(&mut self, conf: Config) {
        self.config = Some(conf);
    }

    pub fn next_wrbpod_session_id(&mut self) -> u128 {
        let next_id = self.next_wrbpod_session_id;
        self.next_wrbpod_session_id += 1;
        next_id
    }

    pub fn add_wrbpod_session(
        &mut self,
        session_id: u128,
        wrbpod_addr: WrbpodAddress,
        session: Wrbpod,
    ) {
        wrb_debug!("Wrbpod session for {} is {}", &wrbpod_addr, session_id);
        self.wrbpod_sessions.insert(session_id, session);
        self.wrbpod_addr_to_session_id
            .insert(wrbpod_addr, session_id);
    }

    pub fn get_wrbpod_session(&mut self, session_id: u128) -> Option<&mut Wrbpod> {
        self.wrbpod_sessions.get_mut(&session_id)
    }

    pub fn get_wrbpod_session_id_by_address(
        &mut self,
        wrbpod_addr: &WrbpodAddress,
    ) -> Option<u128> {
        let session_id = *self.wrbpod_addr_to_session_id.get(wrbpod_addr)?;
        Some(session_id)
    }

    pub fn get_wrbpod_session_by_address(
        &mut self,
        wrbpod_addr: &WrbpodAddress,
    ) -> Option<&mut Wrbpod> {
        let session_id = self.get_wrbpod_session_id_by_address(wrbpod_addr)?;
        self.get_wrbpod_session(session_id)
    }
}

lazy_static! {
    pub static ref GLOBALS: Mutex<Globals> = Mutex::new(Globals {
        config: None,
        wrbpod_addr_to_session_id: HashMap::new(),
        wrbpod_sessions: HashMap::new(),
        next_wrbpod_session_id: 0,
    });
    pub static ref LOGFILE: Mutex<Option<File>> = Mutex::new(Some(
        File::options()
            .append(true)
            .write(true)
            .open("/dev/stderr")
            .expect("FATAL: failed to open /dev/stderr")
    ));
}

pub fn redirect_logfile(new_path: &str) -> Result<(), io::Error> {
    let new_file = File::options()
        .create(true)
        .append(true)
        .write(true)
        .open(new_path)?;
    match LOGFILE.lock() {
        Ok(mut lf_opt) => lf_opt.replace(new_file),
        Err(_e) => {
            panic!("Logfile mutex poisoned");
        }
    };
    Ok(())
}

pub fn with_logfile<F, R>(func: F) -> Option<R>
where
    F: FnOnce(&mut File) -> R,
{
    match LOGFILE.lock() {
        Ok(mut lf_opt) => lf_opt.as_mut().map(|lf| func(lf)),
        Err(_e) => {
            panic!("Logfile mutex poisoned");
        }
    }
}
