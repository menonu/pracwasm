use cosmwasm_std::{Addr, Deps, StdResult, Uint128};
use cw20_base::contract::query_balance;

pub fn query_token_balance(deps: Deps, token_addr: Addr) -> StdResult<Uint128> {
    let balance = query_balance(deps, token_addr.into())?.balance;

    Ok(balance)
}
