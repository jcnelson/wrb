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

use crate::core::Config;
use crate::core::Globals;

use crate::storage::Wrbpod;
use crate::storage::StackerDBClient;

impl Default for Globals {
    fn default() -> Globals {
        Globals {
            config: None,
            wrbpod_sessions: HashMap::new()
        }
    }
}

impl Globals {
    pub fn new() -> Globals {
        Globals::default()
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

    pub fn add_wrbpod_session(&mut self, session_id: u128, session: Wrbpod) {
        self.wrbpod_sessions.insert(session_id, session);
    }

    pub fn remove_wrbpod_session(&mut self, session_id: u128) {
        self.wrbpod_sessions.remove(&session_id);
    }

    pub fn get_wrbpod_session(&mut self, session_id: u128) -> Option<&mut Wrbpod> {
        self.wrbpod_sessions.get_mut(&session_id)
    }
}
