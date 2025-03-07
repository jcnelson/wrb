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

pub mod http;
pub mod runner;
pub mod site;

use clarity::vm::types::QualifiedContractIdentifier;
use libstackerdb::SlotMetadata;

use crate::runner::bns::BNSError;
use crate::runner::bns::BNSNameOwner;
use crate::runner::bns::BNSNameRecord;
use crate::runner::bns::BNSResolver;
use crate::runner::site::WrbTxtRecord;
use crate::runner::site::WrbTxtRecordV1;
use crate::runner::site::ZonefileResourceRecord;
use crate::runner::Error;
use crate::runner::Runner;

impl BNSNameRecord {
    pub fn from_stackerdb_slot(
        stackerdb_contract_id: QualifiedContractIdentifier,
        slot_metadata: SlotMetadata,
    ) -> Self {
        let wrb_rec: WrbTxtRecord =
            WrbTxtRecordV1::new(stackerdb_contract_id, slot_metadata).into();
        let zonefile_rec: ZonefileResourceRecord = wrb_rec.try_into().unwrap();
        let zonefile_bytes = zonefile_rec.to_string().as_bytes().to_vec();
        BNSNameRecord {
            zonefile: Some(zonefile_bytes),
        }
    }
}

pub struct MockBNSResolver {
    names: HashMap<(String, String), BNSNameRecord>,
    owners: HashMap<(String, String), BNSNameOwner>,
    prices: HashMap<(String, String), u128>,
    errors: HashMap<(String, String), BNSError>,
}

impl MockBNSResolver {
    pub fn new() -> Self {
        Self {
            names: HashMap::new(),
            owners: HashMap::new(),
            prices: HashMap::new(),
            errors: HashMap::new(),
        }
    }

    pub fn add_name_rec(&mut self, name: &str, namespace: &str, name_rec: BNSNameRecord) {
        self.names
            .insert((name.to_string(), namespace.to_string()), name_rec);
    }

    pub fn add_name_owner(&mut self, name: &str, namespace: &str, name_owner: BNSNameOwner) {
        self.owners
            .insert((name.to_string(), namespace.to_string()), name_owner);
    }

    pub fn add_name_price(&mut self, name: &str, namespace: &str, price: u128) {
        self.prices
            .insert((name.to_string(), namespace.to_string()), price);
    }

    pub fn add_error(&mut self, name: &str, namespace: &str, error: BNSError) {
        self.errors
            .insert((name.to_string(), namespace.to_string()), error);
    }
}

impl BNSResolver for MockBNSResolver {
    fn lookup(
        &mut self,
        _runner: &mut Runner,
        name: &str,
        namespace: &str,
    ) -> Result<Result<BNSNameRecord, BNSError>, Error> {
        let key = (name.to_string(), namespace.to_string());
        if let Some(err) = self.errors.get(&key) {
            return Ok(Err(err.clone()));
        };
        if let Some(res) = self.names.get(&key) {
            return Ok(Ok(res.clone()));
        }
        return Err(Error::NotConnected);
    }

    fn get_owner(
        &mut self,
        _runner: &mut Runner,
        name: &str,
        namespace: &str,
    ) -> Result<Option<BNSNameOwner>, Error> {
        let key = (name.to_string(), namespace.to_string());
        if let Some(_err) = self.errors.get(&key) {
            return Ok(None);
        };
        if let Some(res) = self.owners.get(&key) {
            return Ok(Some(res.clone()));
        }
        return Err(Error::NotConnected);
    }

    fn get_price(
        &mut self,
        _runner: &mut Runner,
        name: &str,
        namespace: &str,
    ) -> Result<Option<u128>, Error> {
        let key = (name.to_string(), namespace.to_string());
        if let Some(_err) = self.errors.get(&key) {
            return Ok(None);
        };
        if let Some(res) = self.prices.get(&key) {
            return Ok(Some(res.clone()));
        }
        return Err(Error::NotConnected);
    }
}
