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

use crate::core::Config;
use std::path::{Path, PathBuf};
use stacks_common::util::secp256k1::Secp256k1PrivateKey;
use clarity::vm::types::QualifiedContractIdentifier;

use dirs;

pub const WRB_SIGNIN_HELPER: &'static str = "wrb-signin-helper";
pub const WRB_WALLET_HELPER: &'static str = "wrb-wallet-helper";

// maximum size of a compressed attachment is 1MB
pub const MAX_ATTACHMENT_SIZE: usize = 1024 * 1024;

impl Config {
    fn default_helper_programs_dir() -> String {
        let mut wrb_dir = dirs::home_dir().expect("FATAL: could not determine home directory");
        wrb_dir.push(".wrb");
        wrb_dir.push("helpers");

        wrb_dir
            .to_str()
            .expect("FATAL: could not encode home directory path as string")
            .to_string()
    }

    fn default_signin_helper<P: AsRef<Path>>(helper_programs_dir: P) -> String {
        let mut pb = PathBuf::new();
        pb.push(helper_programs_dir);
        pb.push(WRB_SIGNIN_HELPER);

        pb.to_str()
            .expect("FATAL: could not encode path to signin helper")
            .to_string()
    }

    fn default_wallet_helper<P: AsRef<Path>>(helper_programs_dir: P) -> String {
        let mut pb = PathBuf::new();
        pb.push(helper_programs_dir);
        pb.push(WRB_WALLET_HELPER);

        pb.to_str()
            .expect("FATAL: could not encode path to wallet helper")
            .to_string()
    }

    pub fn default(mainnet: bool, node_host: &str, node_port: u16) -> Config {
        let helper_programs_dir = Config::default_helper_programs_dir();
        Config {
            helper_programs_dir: helper_programs_dir.clone(),
            wrb_wallet_helper: Config::default_wallet_helper(&helper_programs_dir),
            wrb_signin_helper: Config::default_signin_helper(&helper_programs_dir),
            mainnet,
            max_attachment_size: MAX_ATTACHMENT_SIZE,
            num_columns: 120,
            node_host: node_host.into(),
            node_port,
            private_key: Secp256k1PrivateKey::new()
        }
    }

    pub fn get_signin_helper(&self) -> &str {
        &self.wrb_signin_helper
    }

    pub fn get_wallet_helper(&self) -> &str {
        &self.wrb_wallet_helper
    }

    pub fn mainnet(&self) -> bool {
        self.mainnet
    }

    pub fn get_node_addr(&self) -> (String, u16) {
        (self.node_host.clone(), self.node_port)
    }

    pub fn private_key(&self) -> &Secp256k1PrivateKey {
        &self.private_key
    }

    pub fn get_bns_contract_id(&self) -> QualifiedContractIdentifier {
        if self.mainnet {
            QualifiedContractIdentifier::parse("SP000000000000000000002Q6VF78.bns").unwrap()
        }
        else {
            QualifiedContractIdentifier::parse("ST000000000000000000002AMW42H.bns").unwrap()
        }
    }
}
