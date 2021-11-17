use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// which cw20 token we send
    pub token: Addr,
    /// how much we send on request
    pub amount: Uint128,
    /// max times we pay out for a given account, default 1
    pub max_requests: u32,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const CALLS: Map<&Addr, u32> = Map::new("calls");
