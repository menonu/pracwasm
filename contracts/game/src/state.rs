use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

use crate::card::Hand;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub owner: Addr,
}

pub const STATE: Item<State> = Item::new("state");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub token_address: Addr,
}

pub const CONFIG: Item<Config> = Item::new("config");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Vault {
    pub balance: Uint128,
}

pub const VAULT: Map<&Addr, Vault> = Map::new("vault");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GameState {
    pub ingame: bool,
    pub total_bet_amount: Uint128,
    pub dealer_hand: Hand,
    pub player_hand: Hand,
}

pub const GAMESTATE: Map<&Addr, GameState> = Map::new("gamestate");
