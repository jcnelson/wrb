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

use clarity::vm::types::QualifiedContractIdentifier;
use stacks_common::util::secp256k1::Secp256k1PrivateKey;
use std::convert::TryFrom;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::Serialize;
use toml;

use crate::storage::WrbpodAddress;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// mainnet or testnet
    mainnet: bool,
    /// node host
    node_host: String,
    /// node port
    node_port: u16,
    /// identity key
    private_key: Secp256k1PrivateKey,
    /// location where we store Wrb DBs
    /// (relative or absolute)
    storage: String,
    /// location of the debug file
    debug_path: String,
    /// our wrbpod
    wrbpod: WrbpodAddress,
    /// Path from which we loaded this
    __path: String,
}

#[derive(Serialize, Deserialize)]
pub struct ConfigFile {
    /// mainnet or testnet
    mainnet: bool,
    /// node host
    node_host: String,
    /// node port
    node_port: u16,
    /// identity key
    private_key: String,
    /// location where we store Wrb DBs
    /// (relative or absolute)
    storage: Option<String>,
    /// location of the debug file
    debug_path: Option<String>,
    /// our wrbpod, encoded as <address>.<name>/<slot-id>
    wrbpod: String,
}

impl ConfigFile {
    pub fn from_str(content: &str) -> Result<Self, String> {
        let config = toml::from_str(content).map_err(|e| format!("Invalid toml: {}", e))?;
        Ok(config)
    }
}

impl TryFrom<ConfigFile> for Config {
    type Error = String;
    fn try_from(config_file: ConfigFile) -> Result<Self, Self::Error> {
        let default_wrbpod = WrbpodAddress::parse(&config_file.wrbpod).ok_or_else(|| {
            format!(
                "Failed to decode '{}': expected <addr>.<name>/<slot-id>",
                &config_file.wrbpod
            )
        })?;

        Ok(Config {
            mainnet: config_file.mainnet,
            node_host: config_file.node_host,
            node_port: config_file.node_port,
            private_key: Secp256k1PrivateKey::from_hex(&config_file.private_key)
                .map_err(|e| format!("Failed to parse `private_key`: {:?}", &e))?,
            storage: config_file.storage.unwrap_or("./db".into()),
            debug_path: config_file.debug_path.unwrap_or("./debug.log".into()),
            wrbpod: default_wrbpod,
            __path: "".into(),
        })
    }
}

impl From<Config> for ConfigFile {
    fn from(config: Config) -> Self {
        Self {
            mainnet: config.mainnet,
            node_host: config.node_host.clone(),
            node_port: config.node_port,
            private_key: config.private_key.to_hex(),
            storage: Some(config.storage),
            debug_path: Some(config.debug_path),
            wrbpod: config.wrbpod.to_string(),
        }
    }
}

impl Config {
    pub fn default(mainnet: bool, node_host: &str, node_port: u16) -> Config {
        Config {
            mainnet,
            node_host: node_host.into(),
            node_port,
            private_key: Secp256k1PrivateKey::new(),
            storage: "./db".into(),
            debug_path: "./debug.log".into(),
            wrbpod: WrbpodAddress::new(
                QualifiedContractIdentifier::parse(
                    "SP000000000000000000002Q6VF78.you-need-to-set-up-your-wrbpod",
                )
                .unwrap(),
                0,
            ),
            __path: "".into(),
        }
    }

    pub fn from_path(path: &str) -> Result<Config, String> {
        let content = fs::read_to_string(path).map_err(|e| format!("Invalid path: {}", &e))?;
        let config_file = ConfigFile::from_str(&content)?;
        let mut c = Config::try_from(config_file)?;
        c.__path = path.into();
        Ok(c)
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

    pub fn default_wrbpod(&self) -> &WrbpodAddress {
        &self.wrbpod
    }

    /// This is the contract ID of the BNS contract that can resolve a name to its owner and price.
    pub fn get_bns_contract_id(&self) -> QualifiedContractIdentifier {
        if self.mainnet {
            QualifiedContractIdentifier::parse("SP2QEZ06AGJ3RKJPBV14SY1V5BBFNAW33D96YPGZF.BNS-V2")
                .unwrap()
        } else {
            // private key: e89bb394ecd5161007a84b34ac98d4f7239016c91d3e0c7c3b97aa499693288301
            QualifiedContractIdentifier::parse("ST1V5THTGSFT6Z793AT7M2H18G3Y9EGVJZNH5E2BG.BNS-V2")
                .unwrap()
        }
    }

    /// This is the contract ID of the BNS contract that can resolve a name to a zonefile.
    pub fn get_zonefile_contract_id(&self) -> QualifiedContractIdentifier {
        if self.mainnet {
            QualifiedContractIdentifier::parse(
                "SP2QEZ06AGJ3RKJPBV14SY1V5BBFNAW33D96YPGZF.zonefile-resolver",
            )
            .unwrap()
        } else {
            // private key: e89bb394ecd5161007a84b34ac98d4f7239016c91d3e0c7c3b97aa499693288301
            QualifiedContractIdentifier::parse(
                "ST1V5THTGSFT6Z793AT7M2H18G3Y9EGVJZNH5E2BG.zonefile-resolver",
            )
            .unwrap()
        }
    }

    pub fn db_path(&self) -> String {
        if let Some('/') = self.storage.chars().next() {
            // absolute path
            self.storage.clone()
        } else {
            // relative path
            if let Some(dirname) = Path::new(&self.__path).parent() {
                format!("{}/{}", dirname.display(), &self.storage)
            } else {
                self.storage.clone()
            }
        }
    }

    pub fn debug_path(&self) -> String {
        if let Some('/') = self.debug_path.chars().next() {
            // absolute path
            self.debug_path.clone()
        } else {
            // relative path
            if let Some(dirname) = Path::new(&self.__path).parent() {
                format!("{}/{}", dirname.display(), &self.debug_path)
            } else {
                self.debug_path.clone()
            }
        }
    }
}
