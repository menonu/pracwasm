#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Storage, Uint128,
};
use cw2::set_contract_version;
use cw20::Cw20ReceiveMsg;

use crate::card::hand_to_string;
use crate::error::ContractError;
use crate::game::dealer_action;
use crate::msg::{
    ActionCommand, Cw20HookMsg, DepositResponse, ExecuteMsg, GameStateResponce,
    InstantiateMsg, QueryMsg,
};
use crate::state::{Config, GameState, State, Vault, CONFIG, GAMESTATE, STATE, VAULT};
use crate::{game, random};

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
        owner: info.sender.clone(),
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

    let token_address = deps.api.addr_validate(&msg.cw20_address)?;
    let config = Config {
        token_address: token_address.clone(),
    };
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender)
        .add_attribute("cw20_address", token_address))
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
        ExecuteMsg::Bet { amount } => try_bet(deps, _env, info, amount),
        ExecuteMsg::Action { action } => try_action(deps, _env, info, action),
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
    if amount < Uint128::new(0) {
        return Err(ContractError::BetAmountZero {});
    }

    let balance_after = exec_bet(deps.storage, &info, amount)?;

    let deal = game::first_deal(&mut random::gen_rng(env.block.time));
    let hand_dealer: String = deal.0.iter().map(|c| c.to_string() + " ").collect();
    let hand_player: String = deal.1.iter().map(|c| c.to_string() + " ").collect();

    let state_after = GAMESTATE.update(deps.storage, &info.sender, |state| {
        let new_game = GameState {
            ingame: true,
            total_bet_amount: amount,
            dealer_hand: deal.0,
            player_hand: deal.1,
        };
        match state {
            Some(v) => {
                if v.ingame {
                    return Err(ContractError::BetAfterStart {});
                }

                Ok(new_game)
            }
            None => Ok(new_game),
        }
    })?;

    Ok(Response::new()
        .add_attribute("action", "bet")
        .add_attribute("bet_amount", state_after.total_bet_amount)
        .add_attribute("balance_after", balance_after.balance)
        .add_attribute("dealer_cards", hand_dealer)
        .add_attribute("player_cards", hand_player))
}

pub fn try_action(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    action: ActionCommand,
) -> Result<Response, ContractError> {
    let mut game = GAMESTATE
        .load(deps.storage, &info.sender)
        .map_err(|_| ContractError::NoSuchAccountExists {})?;

    if !game.ingame {
        return Err(ContractError::ActionBeforeBet {});
    }

    match action {
        ActionCommand::Hit => {
            let draw = game::draw_one(&mut random::gen_rng(env.block.time));
            game.player_hand.push(draw);

            // check busted or not
            if let Judge::PlayerBusted(_) = game::judge(&[], &game.player_hand) {
                // continue
            } else {
                GAMESTATE.save(deps.storage, &info.sender, &game)?;

                return Ok(Response::new()
                    .add_attribute("action", "hit")
                    .add_attribute("dealer_cards", hand_to_string(&game.dealer_hand))
                    .add_attribute("player_cards", hand_to_string(&game.player_hand))
                    .add_attribute("draw", draw.to_string()));
            }
        }
        ActionCommand::DoubleDown { amount } => {
            // raise, draw one, then close game
            if amount != game.total_bet_amount {
                return Err(ContractError::WrongDoublDownAmount {
                    amount: game.total_bet_amount,
                });
            }

            if game.player_hand.len() != 2 {
                return Err(ContractError::DoubleDownNotAllowed {});
            }

            let _ = exec_bet(deps.storage, &info, amount)?;

            game.total_bet_amount += amount;
            game.player_hand
                .push(game::draw_one(&mut random::gen_rng(env.block.time)));
        }
        ActionCommand::Stand => {
            // do nothing, close game
        }
    }

    use game::{GameResult, Judge};

    // dealer draw if player is not busted
    let new_dealer_hand = if let Judge::PlayerBusted(_) = game::judge(&[], &game.player_hand) {
        game.dealer_hand
    } else {
        dealer_action(&game.dealer_hand, &mut random::gen_rng(env.block.time))
    };

    let judge = game::judge(&new_dealer_hand, &game.player_hand);

    let result = match judge {
        Judge::DealerBusted(_) => GameResult::Win,
        Judge::PlayerBusted(_) => GameResult::Loose,
        Judge::DealerWin(_, _) => GameResult::Loose,
        Judge::PlayerWin(_, _) => GameResult::Win,
        Judge::PlayerBJWin(_, _) => GameResult::Win,
        Judge::Draw(_, _) => GameResult::Draw,
    };

    // change balance
    let balance_change = match result {
        GameResult::Win => game.total_bet_amount.saturating_mul(Uint128::new(2)),
        GameResult::Loose => Uint128::new(0),
        GameResult::Draw => game.total_bet_amount,
    };

    VAULT.update(
        deps.storage,
        &info.sender,
        |vault: Option<Vault>| -> StdResult<Vault> {
            let mut vault = vault.unwrap();
            vault.balance = vault.balance.saturating_add(balance_change);
            Ok(vault)
        },
    )?;

    // update state
    game.ingame = false;
    game.dealer_hand = new_dealer_hand;

    GAMESTATE.save(deps.storage, &info.sender, &game)?;

    Ok(Response::new()
        .add_attribute(
            "action",
            match action {
                ActionCommand::Stand => "stand",
                ActionCommand::Hit => "hit",
                ActionCommand::DoubleDown { amount: _ } => "doubledown",
            },
        )
        .add_attribute("state", "end")
        .add_attribute("result", result.to_string())
        .add_attribute("balance_change", balance_change)
        .add_attribute("judge", judge.to_string()))
}

fn exec_bet(
    storage: &mut dyn Storage,
    info: &MessageInfo,
    amount: Uint128,
) -> Result<Vault, ContractError> {
    VAULT.update(storage, &info.sender, |vault| match vault {
        Some(mut v) => {
            if amount > v.balance {
                return Err(ContractError::ShortBalance { balance: v.balance });
            }

            v.balance -= amount;
            Ok(v)
        }
        None => Err(ContractError::NoSuchAccountExists {}),
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetDeposit { address } => to_binary(&query_deposit(deps, address)?),
        QueryMsg::GetGameState { address } => to_binary(&query_gamestate(deps, address)?),
    }
}

fn query_deposit(deps: Deps, address: String) -> StdResult<DepositResponse> {
    let address = deps.api.addr_validate(&address)?;
    let vault = VAULT.may_load(deps.storage, &address)?;
    let deposit = if let Some(k) = vault {
        k.balance
    } else {
        Uint128::new(0)
    };
    Ok(DepositResponse {
        address: address.to_string(),
        deposit,
    })
}

fn query_gamestate(deps: Deps, address: String) -> StdResult<GameStateResponce> {
    let address = deps.api.addr_validate(&address)?;
    let state = GAMESTATE.load(deps.storage, &address)?;

    Ok(GameStateResponce { state })
}

#[cfg(test)]
mod tests {
    use crate::card::BJCard::*;
    use crate::card::Hand;
    use crate::msg::GameStateResponce;

    use super::*;
    use cosmwasm_std::testing::{
        mock_dependencies, mock_dependencies_with_balance, mock_env, mock_info, MockApi,
        MockQuerier, MockStorage,
    };
    use cosmwasm_std::{coins, from_binary, Empty, OwnedDeps};

    fn init_with_balance() -> OwnedDeps<MockStorage, MockApi, MockQuerier, Empty> {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {
            cw20_address: "token0000".to_string(),
        };
        let info = mock_info("creator", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // deposit
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: "user0000".to_string(),
            amount: Uint128::new(1000),
            msg: to_binary(&Cw20HookMsg::Deposit {}).unwrap(),
        });
        let _res = execute(deps.as_mut(), mock_env(), mock_info("token0000", &[]), msg).unwrap();

        deps
    }

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {
            cw20_address: "token0000".to_string(),
        };
        let info = mock_info("creator", &[]);

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn deposit() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            cw20_address: "token0000".to_string(),
        };
        let info = mock_info("creator", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // deposit
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: "user0000".to_string(),
            amount: Uint128::new(1000),
            msg: to_binary(&Cw20HookMsg::Deposit {}).unwrap(),
        });
        let _res = execute(deps.as_mut(), mock_env(), mock_info("token0000", &[]), msg).unwrap();

        let msg = QueryMsg::GetDeposit {
            address: "user0000".to_string(),
        };
        let query_deposit: DepositResponse =
            from_binary(&query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();

        assert_eq!(
            DepositResponse {
                address: "user0000".to_string(),
                deposit: Uint128::new(1000)
            },
            query_deposit
        );

        // deposit again
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: "user0000".to_string(),
            amount: Uint128::new(1000),
            msg: to_binary(&Cw20HookMsg::Deposit {}).unwrap(),
        });
        let _res = execute(deps.as_mut(), mock_env(), mock_info("token0000", &[]), msg).unwrap();
        let msg = QueryMsg::GetDeposit {
            address: "user0000".to_string(),
        };
        let query_deposit: DepositResponse =
            from_binary(&query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();

        assert_eq!(
            DepositResponse {
                address: "user0000".to_string(),
                deposit: Uint128::new(2000)
            },
            query_deposit
        );

        let msg = QueryMsg::GetDeposit {
            address: "other0000".to_string(),
        };
        let query_deposit: DepositResponse =
            from_binary(&query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();
        // non-existing user
        assert_eq!(
            DepositResponse {
                address: "other0000".to_string(),
                deposit: Uint128::new(0)
            },
            query_deposit
        );
    }

    #[test]
    fn bet() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            cw20_address: "token0000".to_string(),
        };
        let info = mock_info("creator", &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // deposit
        let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: "user0000".to_string(),
            amount: Uint128::new(1000),
            msg: to_binary(&Cw20HookMsg::Deposit {}).unwrap(),
        });
        let _res = execute(deps.as_mut(), mock_env(), mock_info("token0000", &[]), msg).unwrap();

        // bet
        let msg = ExecuteMsg::Bet {
            amount: Uint128::new(100),
        };
        let _res = execute(deps.as_mut(), mock_env(), mock_info("user0000", &[]), msg).unwrap();
        let msg = QueryMsg::GetDeposit {
            address: "user0000".to_string(),
        };
        let query_deposit: DepositResponse =
            from_binary(&query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();
        assert_eq!(Uint128::new(900), query_deposit.deposit);

        // bet is not allowed while in game
        let msg = ExecuteMsg::Bet {
            amount: Uint128::new(100),
        };
        let res = execute(deps.as_mut(), mock_env(), mock_info("user0000", &[]), msg).unwrap_err();
        assert_eq!(ContractError::BetAfterStart {}, res);

        // bet more than user's deposit is not allowed.
        let mut deps = init_with_balance();

        let msg = ExecuteMsg::Bet {
            amount: Uint128::new(1001),
        };
        let res = execute(deps.as_mut(), mock_env(), mock_info("user0000", &[]), msg).unwrap_err();
        assert_eq!(
            ContractError::ShortBalance {
                balance: Uint128::new(1000)
            },
            res
        );

        // other0000 fail to bet
        let msg = ExecuteMsg::Bet {
            amount: Uint128::new(100),
        };
        let res = execute(deps.as_mut(), mock_env(), mock_info("other0000", &[]), msg).unwrap_err();
        assert_eq!(ContractError::NoSuchAccountExists {}, res);
    }

    #[test]
    fn action() {
        let mut deps = init_with_balance();

        // action before bet is not allowed
        let msg = ExecuteMsg::Action {
            action: ActionCommand::Stand,
        };
        let ret = execute(deps.as_mut(), mock_env(), mock_info("user0000", &[]), msg).unwrap_err();
        assert_eq!(ContractError::NoSuchAccountExists {}, ret);

        let msg = ExecuteMsg::Bet {
            amount: Uint128::new(100),
        };
        let _ = execute(deps.as_mut(), mock_env(), mock_info("user0000", &[]), msg).unwrap();

        // other0000 fail to action
        let msg = ExecuteMsg::Action {
            action: ActionCommand::Stand,
        };
        let res = execute(deps.as_mut(), mock_env(), mock_info("other0000", &[]), msg).unwrap_err();
        assert_eq!(ContractError::NoSuchAccountExists {}, res);

        let msg = ExecuteMsg::Action {
            action: ActionCommand::Stand,
        };
        let _ret = execute(deps.as_mut(), mock_env(), mock_info("user0000", &[]), msg).unwrap();
    }

    struct CreateOption {
        d: Hand,
        p: Hand,
        amount: u128,
        ingame: bool,
    }

    impl Default for CreateOption {
        fn default() -> Self {
            Self {
                d: vec![],
                p: vec![],
                amount: 100,
                ingame: true,
            }
        }
    }

    fn create_game_storage(mut s: MockStorage, option: CreateOption) -> MockStorage {
        let state = GameState {
            ingame: option.ingame,
            total_bet_amount: Uint128::new(option.amount),
            dealer_hand: option.d,
            player_hand: option.p,
        };

        GAMESTATE
            .save(&mut s, &Addr::unchecked("user0000"), &state)
            .unwrap();
        s
    }

    #[test]
    fn action_stand() {
        let mut deps = init_with_balance();
        deps.storage = create_game_storage(
            deps.storage,
            CreateOption {
                d: vec![Five],
                p: vec![Two, Three],
                ..Default::default()
            },
        );

        // loose
        let msg = ExecuteMsg::Action {
            action: ActionCommand::Stand,
        };
        let _ret = execute(deps.as_mut(), mock_env(), mock_info("user0000", &[]), msg).unwrap();

        let ret: DepositResponse = from_binary(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::GetDeposit {
                    address: "user0000".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(Uint128::new(1000), ret.deposit);

        // stand after stand should be failed
        let msg = ExecuteMsg::Action {
            action: ActionCommand::Stand,
        };
        let ret = execute(deps.as_mut(), mock_env(), mock_info("user0000", &[]), msg).unwrap_err();
        assert_eq!(ContractError::ActionBeforeBet {}, ret);
    }

    #[test]
    fn action_hit() {
        let mut deps = init_with_balance();
        deps.storage = create_game_storage(
            deps.storage,
            CreateOption {
                d: vec![Seven],
                p: vec![Ten, Three],
                ..Default::default()
            },
        );

        let msg = ExecuteMsg::Action {
            action: ActionCommand::Hit,
        };
        let hit = execute(deps.as_mut(), mock_env(), mock_info("user0000", &[]), msg).unwrap();
        let hit_arr_draw = &hit.attributes.get(3).expect("no attribute").value;
        assert_eq!("6", hit_arr_draw);

        let msg = QueryMsg::GetGameState {
            address: "user0000".to_string(),
        };
        let ret = query(deps.as_ref(), mock_env(), msg).unwrap();
        let res: GameStateResponce = from_binary(&ret).unwrap();

        assert_eq!(
            GameState {
                ingame: true,
                total_bet_amount: Uint128::new(100),
                dealer_hand: vec![Seven],
                player_hand: vec![Ten, Three, Six],
            },
            res.state
        );

        // hit after stand/doubledown is not allowed
        let mut deps = init_with_balance();
        deps.storage = create_game_storage(
            deps.storage,
            CreateOption {
                d: vec![Seven],
                p: vec![Ten, Three],
                ..Default::default()
            },
        );
        // stand then hit
        let _ret = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user0000", &[]),
            ExecuteMsg::Action {
                action: ActionCommand::Stand,
            },
        )
        .unwrap();

        let ret = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user0000", &[]),
            ExecuteMsg::Action {
                action: ActionCommand::Hit,
            },
        )
        .unwrap_err();
        assert_eq!(ContractError::ActionBeforeBet {}, ret);

        // doubledown then hit
        let mut deps = init_with_balance();
        deps.storage = create_game_storage(
            deps.storage,
            CreateOption {
                d: vec![Seven],
                p: vec![Ten, Three],
                ..Default::default()
            },
        );

        let _ret = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user0000", &[]),
            ExecuteMsg::Action {
                action: ActionCommand::DoubleDown {
                    amount: Uint128::new(100),
                },
            },
        )
        .unwrap();

        let ret = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("user0000", &[]),
            ExecuteMsg::Action {
                action: ActionCommand::Hit {},
            },
        )
        .unwrap_err();
        assert_eq!(ContractError::ActionBeforeBet {}, ret);
    }

    #[test]
    fn action_doubledown() {
        let mut deps = init_with_balance();
        deps.storage = create_game_storage(
            deps.storage,
            CreateOption {
                d: vec![Seven],
                p: vec![Ten, Three],
                ..Default::default()
            },
        );
        let msg = ExecuteMsg::Action {
            action: ActionCommand::DoubleDown {
                amount: Uint128::new(100),
            },
        };
        let _ = execute(deps.as_mut(), mock_env(), mock_info("user0000", &[]), msg).unwrap();

        let mut deps = init_with_balance();
        deps.storage = create_game_storage(
            deps.storage,
            CreateOption {
                d: vec![Seven],
                p: vec![Ten, Three, Three],
                ..Default::default()
            },
        );
        let msg = ExecuteMsg::Action {
            action: ActionCommand::DoubleDown {
                amount: Uint128::new(100),
            },
        };
        // doubledown after hit is not allowd
        let err = execute(deps.as_mut(), mock_env(), mock_info("user0000", &[]), msg).unwrap_err();
        assert_eq!(ContractError::DoubleDownNotAllowed {}, err);

        let mut deps = init_with_balance();
        deps.storage = create_game_storage(
            deps.storage,
            CreateOption {
                d: vec![Seven],
                p: vec![Ten, Three],
                amount: 1200,
                ..Default::default()
            },
        );
        let msg = ExecuteMsg::Action {
            action: ActionCommand::DoubleDown {
                amount: Uint128::new(1200),
            },
        };
        let err = execute(deps.as_mut(), mock_env(), mock_info("user0000", &[]), msg).unwrap_err();
        assert_eq!(
            ContractError::ShortBalance {
                balance: Uint128::new(1000)
            },
            err
        );

        let mut deps = init_with_balance();
        deps.storage = create_game_storage(
            deps.storage,
            CreateOption {
                d: vec![Seven],
                p: vec![Ten, Three],
                ..Default::default()
            },
        );
        let msg = ExecuteMsg::Action {
            action: ActionCommand::DoubleDown {
                amount: Uint128::new(200),
            },
        };
        let err = execute(deps.as_mut(), mock_env(), mock_info("user0000", &[]), msg).unwrap_err();
        assert_eq!(
            ContractError::WrongDoublDownAmount {
                amount: Uint128::new(100)
            },
            err
        );
    }
}
