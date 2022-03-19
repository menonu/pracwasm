#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Reply, ReplyOn, Response,
    StdError, StdResult, SubMsg, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw20::{self, Cw20ExecuteMsg, MinterResponse};
use cw_utils::parse_instantiate_response_data;

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
        token_address: Addr::unchecked(""),
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
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    let mut state: State = STATE.load(deps.storage)?;

    let res = msg.result.unwrap();
    let data =
        parse_instantiate_response_data(res.data.unwrap_or_default().as_slice()).map_err(|_| {
            StdError::parse_err("MsgInstantiateContractResponse", "failed to parse data")
        })?;

    if state.token_address != Addr::unchecked("") {
        return Err(ContractError::Unauthorized {});
    }

    state.token_address = deps.api.addr_validate(&data.contract_address)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new().add_attribute("token address", state.token_address))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Claim { amount } => try_claim(deps, info, amount),
        ExecuteMsg::TopUp { amount } => try_topup(deps, info, amount),
    }
}

pub fn try_claim(
    deps: DepsMut,
    _info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let token_addr = STATE.load(deps.storage)?.token_address;

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
    amount: Uint128,
) -> Result<Response, ContractError> {
    let token_addr = STATE.load(deps.storage)?.token_address;

    STATE.update(deps.storage, |mut stats| -> Result<_, ContractError> {
        stats.supply = stats.supply.saturating_add(amount);
        Ok(stats)
    })?;

    let msg: CosmosMsg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token_addr.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::Mint {
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
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{
        coins, from_binary, ReplyOn, SubMsg, SubMsgExecutionResponse, SubMsgResult,
    };
    use prost::Message;

    #[derive(Clone, PartialEq, Message)]
    struct MsgInstantiateContractResponse {
        #[prost(string, tag = "1")]
        pub contract_address: ::prost::alloc::string::String,
        #[prost(bytes, tag = "2")]
        pub data: ::prost::alloc::vec::Vec<u8>,
    }

    fn reply_token_address(deps: DepsMut, msg_id: u64, contract_address: String) {
        let data = MsgInstantiateContractResponse {
            contract_address,
            data: vec![],
        };

        let mut encoded_instantiate_reply = Vec::<u8>::with_capacity(data.encoded_len());
        // The data must encode successfully
        data.encode(&mut encoded_instantiate_reply).unwrap();

        // Build reply message
        let msg = Reply {
            id: msg_id,
            result: SubMsgResult::Ok(SubMsgExecutionResponse {
                events: vec![],
                data: Some(encoded_instantiate_reply.into()),
            }),
        };

        let _res = reply(deps, mock_env(), msg.clone()).unwrap();
    }

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
                    })
                    .unwrap(),
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
        let _ = reply_token_address(deps.as_mut(), 1, "asset0000".to_string());

        let info = mock_info("anyone", &[]);
        let msg = ExecuteMsg::Claim {
            amount: Uint128::new(1),
        };
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();

        assert_eq!(res, ContractError::OutOfStock {});

        let info = mock_info("anyone", &[]);
        let msg = ExecuteMsg::Claim {
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
        let _ = reply_token_address(deps.as_mut(), 1, "asset0000".to_string());

        let info = mock_info("someone", &coins(2, "token"));
        let msg = ExecuteMsg::TopUp {
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
                    msg: to_binary(&Cw20ExecuteMsg::Mint {
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
