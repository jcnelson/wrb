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

use crate::runner::Config;
use std::path::{Path, PathBuf};

use dirs;

pub const WRB_SIGNIN_HELPER: &'static str = "wrb-signin-helper";
pub const WRB_GAIA_HELPER: &'static str = "wrb-gaia-helper";
pub const WRB_NODE_HELPER: &'static str = "wrb-node-helper";
pub const WRB_WALLET_HELPER: &'static str = "wrb-wallet-helper";

// maximum size of a compressed attachment is 1MB
pub const MAX_ATTACHMENT_SIZE : usize = 1024 * 1024;

impl Config {
    fn default_helper_programs_dir() -> String {
        let mut wrb_dir = dirs::home_dir().expect("FATAL: could not determine home directory");
        wrb_dir.push(".wrb");
        wrb_dir.push("helpers");

        wrb_dir.to_str().expect("FATAL: could not encode home directory path as string").to_string()
    }

    fn default_signin_helper<P: AsRef<Path>>(helper_programs_dir: P) -> String {
        let mut pb = PathBuf::new();
        pb.push(helper_programs_dir);
        pb.push(WRB_SIGNIN_HELPER);

        pb.to_str().expect("FATAL: could not encode path to signin helper").to_string()
    }
    
    fn default_gaia_helper<P: AsRef<Path>>(helper_programs_dir: P) -> String {
        let mut pb = PathBuf::new();
        pb.push(helper_programs_dir);
        pb.push(WRB_GAIA_HELPER);

        pb.to_str().expect("FATAL: could not encode path to gaia helper").to_string()
    }
    
    fn default_node_helper<P: AsRef<Path>>(helper_programs_dir: P) -> String {
        let mut pb = PathBuf::new();
        pb.push(helper_programs_dir);
        pb.push(WRB_NODE_HELPER);

        pb.to_str().expect("FATAL: could not encode path to node helper").to_string()
    }

    fn default_wallet_helper<P: AsRef<Path>>(helper_programs_dir: P) -> String {
        let mut pb = PathBuf::new();
        pb.push(helper_programs_dir);
        pb.push(WRB_WALLET_HELPER);

        pb.to_str().expect("FATAL: could not encode path to wallet helper").to_string()
    }

    pub fn default(mainnet: bool, node: &str, gaia_hub: &str) -> Config {
        let helper_programs_dir = Config::default_helper_programs_dir();
        Config {
            helper_programs_dir: helper_programs_dir.clone(),
            wrb_node_helper: Config::default_node_helper(&helper_programs_dir),
            wrb_gaia_helper: Config::default_gaia_helper(&helper_programs_dir),
            wrb_wallet_helper: Config::default_wallet_helper(&helper_programs_dir),
            wrb_signin_helper: Config::default_signin_helper(&helper_programs_dir),
            node_url: node.to_string(),
            gaia_url: gaia_hub.to_string(),
            mainnet,
            max_attachment_size: MAX_ATTACHMENT_SIZE,
            num_columns: 120
        }
    }

    pub fn get_signin_helper(&self) -> &str {
        &self.wrb_signin_helper
    }

    pub fn get_node_helper(&self) -> &str {
        &self.wrb_node_helper
    }

    pub fn get_wallet_helper(&self) -> &str {
        &self.wrb_wallet_helper
    }

    pub fn get_gaia_helper(&self) -> &str {
        &self.wrb_gaia_helper
    }

    pub fn get_node_url(&self) -> &str {
        &self.node_url
    }

    pub fn get_gaia_url(&self) -> &str {
        &self.gaia_url
    }

    pub fn mainnet(&self) -> bool {
        self.mainnet
    }
}
