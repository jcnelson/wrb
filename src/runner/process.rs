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
use std::convert::TryFrom;
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use std::io::Write;

use crate::core::Config;
use crate::runner::Error;
use crate::runner::Runner;

use clarity::vm::types::BufferLength;
use clarity::vm::types::PrincipalData;
use clarity::vm::types::QualifiedContractIdentifier;
use clarity::vm::types::SequenceData;
use clarity::vm::types::Value;

use stacks_common::util::hash::Hash160;

use serde::Deserialize;
use serde::Serialize;
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
    pub fn new(
        bns_contract_id: QualifiedContractIdentifier,
        zonefile_contract_id: QualifiedContractIdentifier,
        node_host: String,
        node_port: u16,
    ) -> Runner {
        Runner {
            bns_contract_id,
            zonefile_contract_id,
            node_host,
            node_port,
            node: None,
        }
    }

    /// Returns (exit code, stdout, stderr)
    fn inner_run_process(
        &self,
        bin_fullpath: &str,
        args: &[&str],
        _stdin: Option<String>,
    ) -> Result<(i32, Vec<u8>, Vec<u8>), Error> {
        let full_args = fmt_bin_args(bin_fullpath, args);
        wrb_debug!("Run: `{}`", &full_args);
        let cmd = Command::new(bin_fullpath)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .args(args)
            .spawn()
            .map_err(|e| {
                wrb_warn!("Failed to run '{}': {:?}", &full_args, &e);
                Error::FailedToRun(full_args.clone(), vec![])
            })?;

        let output = cmd
            .wait_with_output()
            .map_err(|ioe| Error::FailedToExecute(full_args, format!("{}", &ioe)))?;

        let exit_code = match output.status.code() {
            Some(code) => code,
            None => {
                // failed due to signal
                let full_args = fmt_bin_args(bin_fullpath, args);
                wrb_warn!("Failed to run '{}': killed by signal", &bin_fullpath);
                return Err(Error::KilledBySignal(full_args));
            }
        };

        Ok((exit_code, output.stdout, output.stderr))
    }
}
