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

use std::process::{Command, Stdio};
use std::env;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use std::io::Write;

use crate::runner::Config;
use crate::runner::Runner;
use crate::runner::Error;

use serde_json;

fn fmt_bin_args(bin: &str, args: &[&str]) -> String {
    let mut all = Vec::with_capacity(1 + args.len());
    all.push(bin);
    for arg in args {
        all.push(arg);
    }
    all.join(" ")
}

impl Runner {
    pub fn new(config: Config) -> Runner {
        Runner {
            config
        }
    }

    /// Returns (exit code, stdout, stderr)
    fn inner_run_process(&self, bin_fullpath: &str, args: &[&str], stdin: Option<String>) -> Result<(i32, Vec<u8>, Vec<u8>), Error> {
        let full_args = fmt_bin_args(bin_fullpath, args);
        let cmd = Command::new(bin_fullpath)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .args(args)
            .spawn()
            .map_err(|e| {
                warn!("Failed to run '{}': {:?}", &full_args, &e);
                Error::FailedToRun(full_args.clone())
            })?;

        let output = cmd.wait_with_output()
            .map_err(|ioe| Error::FailedToExecute(full_args, ioe))?;

        let exit_code = match output.status.code() {
            Some(code) => code,
            None => {
                // failed due to signal
                let full_args = fmt_bin_args(bin_fullpath, args);
                warn!("Failed to run '{}': killed by signal", &bin_fullpath);
                return Err(Error::KilledBySignal(full_args));
            }
        };
        
        Ok((exit_code, output.stdout, output.stderr))
    }

    /// Generate a BIP39 seed phrase via `wrb-wallet-helper seed-phrase`
    pub fn wallet_seed_phrase(&self) -> Result<String, Error> {
        let fp = self.config.get_wallet_helper();
        let (exit_status, stdout, _stderr) = self.inner_run_process(fp, &["seed-phrase"], None)?;
        if exit_status != 0 {
            return Err(Error::BadExit(exit_status));
        }

        // decode -- should be a JSON string
        let stdout_str = std::str::from_utf8(&stdout)
            .map_err(|_| Error::InvalidOutput("<corrupt-seed-phrase>".to_string()))?;

        let phrase : serde_json::Value = serde_json::from_str(stdout_str)
            .map_err(|_| Error::InvalidOutput(stdout_str.to_string()))?;

        let phrase = phrase.as_str()
            .ok_or(Error::InvalidOutput(stdout_str.to_string()))?
            .to_string();

        Ok(phrase)
    }

    /// Go and get a BNS name's zonefile
    pub fn gaia_get_zonefile(&self, bns_name: &str) -> Result<Vec<u8>, Error> {
        let fp = self.config.get_gaia_helper();
        let node_url = self.config.get_node_url();
        let args = if self.config.mainnet() {
            vec!["-n", node_url, "-r", bns_name]
        }
        else {
            vec!["-t", "-n", node_url, "-r", bns_name]
        };

        let (exit_status, stdout, _stderr) = self.inner_run_process(fp, &args, None)?;
        if exit_status != 0 {
            return Err(Error::BadExit(exit_status));
        }

        Ok(stdout)
    }
}
