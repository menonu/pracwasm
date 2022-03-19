use cosmwasm_std::{to_binary, Addr, Deps, QueryRequest, StdResult, Uint128, WasmQuery};
use cw20::{BalanceResponse, Cw20QueryMsg};

pub fn query_token_balance(deps: Deps, token_address: Addr, address: Addr) -> StdResult<Uint128> {
    let res: BalanceResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: String::from(token_address),
        msg: to_binary(&Cw20QueryMsg::Balance {
            address: address.to_string(),
        })?,
    }))?;

    Ok(res.balance)
}
