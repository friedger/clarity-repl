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

use std::cmp;
use std::convert::{TryFrom, TryInto};

use crate::clarity::contexts::GlobalContext;
use crate::clarity::errors::Error;
use crate::clarity::errors::{CheckErrors, InterpreterError, InterpreterResult as Result, RuntimeErrorType};
use crate::clarity::representations::{SymbolicExpression, SymbolicExpressionType};
use crate::clarity::types::{PrincipalData, QualifiedContractIdentifier, Value};

use chainstate::stacks::boot::boot_code_id;
use chainstate::stacks::db::StacksChainState;
use chainstate::stacks::events::{STXEventType, STXLockEventData, StacksTransactionEvent};
use crate::clarity::clarity::ClarityTransactionConnection;

fn parse_pox_stacking_result(
    result: &Value,
) -> std::result::Result<(PrincipalData, u128, u64), i128> {
    match result.clone().expect_result() {
        Ok(res) => {
            // should have gotten back (ok (tuple (stacker principal) (lock-amount uint) (unlock-burn-height uint)))
            let tuple_data = res.expect_tuple();
            let stacker = tuple_data
                .get("stacker")
                .expect(&format!("FATAL: no 'stacker'"))
                .to_owned()
                .expect_principal();

            let lock_amount = tuple_data
                .get("lock-amount")
                .expect(&format!("FATAL: no 'lock-amount'"))
                .to_owned()
                .expect_u128();

            let unlock_burn_height = tuple_data
                .get("unlock-burn-height")
                .expect(&format!("FATAL: no 'unlock-burn-height'"))
                .to_owned()
                .expect_u128()
                .try_into()
                .expect("FATAL: 'unlock-burn-height' overflow");

            Ok((stacker, lock_amount, unlock_burn_height))
        }
        Err(e) => Err(e.expect_i128()),
    }
}

/// Handle special cases when calling into the PoX API contract
fn handle_pox_api_contract_call(
    global_context: &mut GlobalContext,
    _sender_opt: Option<&PrincipalData>,
    function_name: &str,
    value: &Value,
) -> Result<()> {
    if function_name == "stack-stx" || function_name == "delegate-stack-stx" {
        debug!(
            "Handle special-case contract-call to {:?} {} (which returned {:?})",
            boot_code_id("pox"),
            function_name,
            value
        );

        match parse_pox_stacking_result(value) {
            Ok((stacker, locked_amount, unlock_height)) => {
                // if this fails, then there's a bug in the contract (since it already does
                // the necessary checks)
                match StacksChainState::pox_lock(
                    &mut global_context.database,
                    &stacker,
                    locked_amount,
                    unlock_height as u64,
                ) {
                    Ok(_) => {
                        if let Some(batch) = global_context.event_batches.last_mut() {
                            batch.events.push(StacksTransactionEvent::STXEvent(
                                STXEventType::STXLockEvent(STXLockEventData {
                                    locked_amount,
                                    unlock_height,
                                    locked_address: stacker,
                                }),
                            ));
                        }
                    }
                    Err(e) => {
                        panic!(
                            "FATAL: failed to lock {} from {} until {}: '{:?}'",
                            locked_amount, stacker, unlock_height, &e
                        );
                    }
                }

                return Ok(());
            }
            Err(_) => {
                // nothing to do -- the function failed
                return Ok(());
            }
        }
    }
    // nothing to do
    Ok(())
}

/// Handle special cases of contract-calls -- namely, those into PoX that should lock up STX
pub fn handle_contract_call_special_cases(
    global_context: &mut GlobalContext,
    sender: Option<&PrincipalData>,
    contract_id: &QualifiedContractIdentifier,
    function_name: &str,
    result: &Value,
) -> Result<()> {
    if *contract_id == boot_code_id("pox") {
        return handle_pox_api_contract_call(global_context, sender, function_name, result);
    }
    // TODO: insert more special cases here, as needed
    Ok(())
}
