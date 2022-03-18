#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, ReplyOn, Response, StdResult,
    SubMsg, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw20::{self, Cw20ExecuteMsg, MinterResponse};

use crate::error::ContractError;
use crate::msg::{ClaimedResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::querier;
use crate::state::{State, Stats, STATE, STATS};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:pracwasm";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const SUBMESSAGE_REPLY_ID: u64 = 1;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let state = State {
        owner: info.sender.clone(),
        supply: Uint128::new(0),
    };
    STATE.save(deps.storage, &state)?;

    let stats = Stats {
        claimed: msg.claimed,
    };
    STATS.save(deps.storage, &stats)?;

    let sub_msg = SubMsg {
        msg: WasmMsg::Instantiate {
            admin: None,
            code_id: msg.cw20_code_id,
            msg: to_binary(&cw20_base::msg::InstantiateMsg {
                name: msg.token_name,
                symbol: msg.token_symbol,
                decimals: 6,
                initial_balances: vec![],
                mint: Some(MinterResponse {
                    minter: env.contract.address.to_string(),
                    cap: None,
                }),
                marketing: None,
            })?,
            funds: vec![],
            label: "BJ token".to_string(),
        }
        .into(),
        id: SUBMESSAGE_REPLY_ID,
        gas_limit: None,
        reply_on: ReplyOn::Success,
    };

    Ok(Response::new()
        .add_submessage(sub_msg)
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender)
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
        ExecuteMsg::TopUp { token, amount } => try_topup(deps, info, token, amount),
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
        .add_attribute("total_claimed", stats.claimed.to_string()))
}

pub fn try_topup(
    deps: DepsMut,
    info: MessageInfo,
    token: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let addr = deps.api.addr_validate(&token)?;

    STATE.update(deps.storage, |mut stats| -> Result<_, ContractError> {
        stats.supply = stats.supply.saturating_add(amount);
        Ok(stats)
    })?;

    let msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: addr.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: info.sender.to_string(),
            amount,
        })?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_message(msg)
        .add_attribute("method", "topup")
        .add_attribute("amount", amount.to_string()))
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
    use cosmwasm_std::testing::{
        mock_dependencies, mock_dependencies_with_balance, mock_env, mock_info,
    };
    use cosmwasm_std::{coins, from_binary, ReplyOn, SubMsg};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {
            claimed: Uint128::from(10u128),
            token_name: "some token".to_string(),
            token_symbol: "some".to_string(),
            cw20_code_id: 5,
        };
        let info = mock_info("anyone", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(1, res.messages.len());

        assert_eq!(
            SubMsg {
                msg: CosmosMsg::Wasm(WasmMsg::Instantiate {
                    admin: None,
                    code_id: 5,
                    msg: to_binary(&cw20_base::msg::InstantiateMsg {
                        name: "some token".to_string(),
                        symbol: "some".to_string(),
                        decimals: 6,
                        initial_balances: vec![],
                        mint: Some(MinterResponse {
                            minter: mock_env().contract.address.to_string(),
                            cap: None,
                        }),
                        marketing: None,
                    }).unwrap(),
                    funds: vec![],
                    label: "BJ token".to_string(),
                }),
                id: 1,
                gas_limit: None,
                reply_on: ReplyOn::Success,
            },
            res.messages[0],
        );

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetClaimed {}).unwrap();
        let value: ClaimedResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(10), value.claimed);
    }

    #[test]
    fn claim() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {
            claimed: Uint128::from(10u128),
            token_name: "some token".to_string(),
            token_symbol: "some".to_string(),
            cw20_code_id: 5,
        };
        let info = mock_info("someone", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("anyone", &[]);
        let msg = ExecuteMsg::Claim {
            token: String::from("asset0000"),
            amount: Uint128::new(1),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(res, ContractError::OutOfStock {});

        let info = mock_info("anyone", &[]);
        let msg = ExecuteMsg::Claim {
            token: String::from("asset0000"),
            amount: Uint128::new(0),
        };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetClaimed {}).unwrap();
        let value: ClaimedResponse = from_binary(&res).unwrap();
        assert_eq!(Uint128::new(10), value.claimed);
    }

    #[test]
    fn topup() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));
        let msg = InstantiateMsg {
            claimed: Uint128::from(0u128),
            token_name: "some token".to_string(),
            token_symbol: "some".to_string(),
            cw20_code_id: 5,
        };
        let info = mock_info("someone", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("someone", &coins(2, "token"));
        let msg = ExecuteMsg::TopUp {
            token: String::from("asset0000"),
            amount: Uint128::new(1000),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res_msg0 = res.messages.get(0).expect("no message");

        let info = mock_info("someone", &coins(2, "token"));
        assert_eq!(
            res_msg0,
            &SubMsg {
                msg: CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: "asset0000".to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: info.sender.to_string(),
                        amount: Uint128::new(1000),
                    })
                    .unwrap(),
                    funds: vec![],
                }),
                id: 0,
                gas_limit: None,
                reply_on: ReplyOn::Never,
            }
        );
    }
}
