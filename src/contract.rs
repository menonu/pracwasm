#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ClaimedResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::querier;
use crate::state::{Stats, STATS};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:pracwasm";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let stats = Stats {
        claimed: msg.claimed,
    };

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    STATS.save(deps.storage, &stats)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        // .add_attribute("owner", info.sender)
        .add_attribute("claimed", msg.claimed.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Claim { token, amount } => try_claim(deps, info, token, amount),
    }
}

pub fn try_claim(
    deps: DepsMut,
    _info: MessageInfo,
    token: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let token_addr = deps.api.addr_validate(&token)?;

    // validate balance
    let balance = querier::query_token_balance(deps.as_ref(), token_addr)?;

    if balance < amount {
        return Err(ContractError::OutOfStock {});
    }

    let stats = STATS.update(deps.storage, |mut stats| -> Result<_, ContractError> {
        stats.claimed = stats.claimed.saturating_add(amount);
        Ok(stats)
    })?;

    Ok(Response::new()
        .add_attribute("method", "claim")
        .add_attribute("amount", amount.to_string())
        .add_attribute("total_claimed", stats.claimed.to_string())
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetClaimed {} => to_binary(&query_claimed(deps)?),
    }
}

fn query_claimed(deps: Deps) -> StdResult<ClaimedResponse> {
    let stats = STATS.load(deps.storage)?;
    Ok(ClaimedResponse {
        claimed: stats.claimed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { claimed: Uint128::from(10u128) };
        let info = mock_info("anyone", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetClaimed {}).unwrap();
        let value: ClaimedResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(10), value.claimed);
    }

    #[test]
    fn claim() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { claimed: Uint128::from(10u128) };
        let info = mock_info("someone", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("anyone", &[]);
        let msg = ExecuteMsg::Claim {token: String::from("wasm1x9ajkd2w3nh"), amount: Uint128::new(1)};
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(res, ContractError::OutOfStock {});

        let info = mock_info("anyone", &[]);
        let msg = ExecuteMsg::Claim {token: String::from("wasm1x9ajkd2w3nh"), amount: Uint128::new(0)};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetClaimed {}).unwrap();
        let value: ClaimedResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(10), value.claimed);
    }
}
