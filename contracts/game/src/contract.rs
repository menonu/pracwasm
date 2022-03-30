#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Uint128,
};
use cw2::set_contract_version;
use cw20::Cw20ReceiveMsg;

use crate::error::ContractError;
use crate::msg::{
    ActionCommand, CountResponse, Cw20HookMsg, DepositResponse, ExecuteMsg, InstantiateMsg,
    QueryMsg,
};
use crate::{random, game};
use crate::state::{Config, GameState, State, Vault, CONFIG, GAMESTATE, STATE, VAULT};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:project-name";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        count: msg.count,
        owner: info.sender.clone(),
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender)
        .add_attribute("count", msg.count.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, info, msg),
        ExecuteMsg::Reset { count } => try_reset(deps, info, count),
        ExecuteMsg::Bet { amount } => try_bet(deps, _env, info, amount),
        ExecuteMsg::Action { action } => try_action(deps, info, action),
    }
}

pub fn receive_cw20(
    deps: DepsMut,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let contract_address = info.sender;
    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Deposit {}) => {
            // validate cw20 contract
            let config: Config = CONFIG.load(deps.storage)?;
            if config.token_address != contract_address {
                return Err(ContractError::Unauthorized {});
            }

            let transfer_amount = cw20_msg.amount;
            let new_vault = VAULT.update(
                deps.storage,
                &Addr::unchecked(cw20_msg.sender),
                |d: Option<Vault>| -> StdResult<Vault> {
                    match d {
                        Some(vault) => Ok(Vault {
                            balance: vault.balance.saturating_add(transfer_amount),
                        }),
                        None => Ok(Vault {
                            balance: transfer_amount,
                        }),
                    }
                },
            )?;

            Ok(Response::new()
                .add_attribute("action", "deposit")
                .add_attribute("amount", new_vault.balance))
        }
        Err(err) => Err(ContractError::Std(err)),
    }
}

pub fn try_reset(deps: DepsMut, info: MessageInfo, count: i32) -> Result<Response, ContractError> {
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        if info.sender != state.owner {
            return Err(ContractError::Unauthorized {});
        }
        state.count = count;
        Ok(state)
    })?;
    Ok(Response::new().add_attribute("method", "reset"))
}

/// User bet against the dealer.
/// fail if bet amount is bigger than deposit.
///
/// The game starts here.
pub fn try_bet(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let state_after = GAMESTATE.update(deps.storage, &info.sender, |state| match state {
        Some(v) => {
            if !v.ingame {
                return Err(ContractError::InvalidState {});
            }

            Ok(GameState::new(amount, 0))
        }
        None => Ok(GameState::new(amount, 0)),
    })?;

    let _card = game::draw_one(&mut random::gen_rng(env.block.time));

    Ok(Response::new()
        .add_attribute("action", "bet")
        .add_attribute("amount", state_after.total_bet_amount))
}

pub fn try_action(
    _deps: DepsMut,
    _info: MessageInfo,
    action: ActionCommand,
) -> Result<Response, ContractError> {
    println!("{:?}", action);

    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetCount {} => to_binary(&query_count(deps)?),
        QueryMsg::GetDeposit { address } => to_binary(&query_deposit(deps, address)?),
    }
}

fn query_count(deps: Deps) -> StdResult<CountResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(CountResponse { count: state.count })
}

fn query_deposit(deps: Deps, address: String) -> StdResult<DepositResponse> {
    let vault = VAULT.may_load(deps.storage, &Addr::unchecked(&address))?;
    let deposit = if let Some(k) = vault {
        k.balance
    } else {
        Uint128::new(0)
    };
    Ok(DepositResponse { address, deposit })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(17, value.count);
    }

    #[test]
    fn reset() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg { count: 17 };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let unauth_info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
        match res {
            Err(ContractError::Unauthorized {}) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // only the original creator can reset the counter
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // should now be 5
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(5, value.count);
    }
}
