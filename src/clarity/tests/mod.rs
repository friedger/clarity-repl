// Copyright (C) 2013-2020 Blocstack PBC, a public benefit corporation
// Copyright (C) 2020 Stacks Open Internet Foundation
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

use util::hash::hex_bytes;
use crate::clarity::contexts::{Environment, GlobalContext, OwnedEnvironment};
use crate::clarity::contracts::Contract;
use crate::clarity::database::{
    ClarityDatabase, MarfedKV, MemoryBackingStore, NULL_BURN_STATE_DB, NULL_HEADER_DB,
};
use crate::clarity::errors::Error;
use crate::clarity::execute as vm_execute;
use crate::clarity::representations::SymbolicExpression;
use crate::clarity::types::{PrincipalData, ResponseData, Value};

use chainstate::stacks::index::storage::TrieFileStorage;
use chainstate::stacks::index::MarfTrieId;
use chainstate::stacks::StacksBlockHeader;
use chainstate::stacks::StacksBlockId;

use core::{FIRST_BURNCHAIN_CONSENSUS_HASH, FIRST_STACKS_BLOCK_HASH};

mod assets;
mod contracts;
pub mod costs;
mod datamaps;
mod defines;
mod events;
mod forking;
mod large_contract;
mod sequences;
mod simple_apply_eval;
mod traits;

pub fn with_memory_environment<F>(f: F, top_level: bool)
where
    F: FnOnce(&mut OwnedEnvironment) -> (),
{
    let mut marf_kv = MemoryBackingStore::new();

    let mut owned_env = OwnedEnvironment::new(marf_kv.as_clarity_db());
    // start an initial transaction.
    if !top_level {
        owned_env.begin();
    }

    f(&mut owned_env)
}

pub fn with_marfed_environment<F>(f: F, top_level: bool)
where
    F: FnOnce(&mut OwnedEnvironment) -> (),
{
    let mut marf_kv = MarfedKV::temporary();
    marf_kv.begin(
        &StacksBlockId::sentinel(),
        &StacksBlockHeader::make_index_block_hash(
            &FIRST_BURNCHAIN_CONSENSUS_HASH,
            &FIRST_STACKS_BLOCK_HASH,
        ),
    );

    {
        marf_kv
            .as_clarity_db(&NULL_HEADER_DB, &NULL_BURN_STATE_DB)
            .initialize();
    }

    marf_kv.test_commit();
    marf_kv.begin(
        &StacksBlockHeader::make_index_block_hash(
            &FIRST_BURNCHAIN_CONSENSUS_HASH,
            &FIRST_STACKS_BLOCK_HASH,
        ),
        &StacksBlockId([1 as u8; 32]),
    );

    {
        let mut owned_env =
            OwnedEnvironment::new(marf_kv.as_clarity_db(&NULL_HEADER_DB, &NULL_BURN_STATE_DB));
        // start an initial transaction.
        if !top_level {
            owned_env.begin();
        }

        f(&mut owned_env)
    }
}

pub fn execute(s: &str) -> Value {
    vm_execute(s).unwrap().unwrap()
}

pub fn symbols_from_values(mut vec: Vec<Value>) -> Vec<SymbolicExpression> {
    vec.drain(..)
        .map(|value| SymbolicExpression::atom_value(value))
        .collect()
}

pub fn is_committed(v: &Value) -> bool {
    eprintln!("is_committed?: {}", v);

    match v {
        Value::Response(ref data) => data.committed,
        _ => false,
    }
}

pub fn is_err_code(v: &Value, e: u128) -> bool {
    eprintln!("is_err_code?: {}", v);
    match v {
        Value::Response(ref data) => !data.committed && *data.data == Value::UInt(e),
        _ => false,
    }
}
