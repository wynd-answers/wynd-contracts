use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    // address of oracle contract (this allows writing data)
    pub oracle: Addr,
    // address of the cw20 token that we use for payment
    pub token: Addr,
    // maximum amount that can be invested in one hex
    pub max_investment_hex: Uint128,
    // how many days the investment takes until maturity (eg. we pay out in the results in 30 days, 180 days)
    pub maturity_days: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct Location {
    pub cur_index: Option<Decimal>,
    // amount of money invested here
    pub total_invested: Uint128,
    pub current_invested: Uint128,
    // number of individual investments made (people)
    pub total_investments: u64,
    pub current_investments: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Investment {
    // how much was invested
    pub amount: Uint128,
    // starting value when investment was created
    pub baseline_index: Decimal,
    // when this investment was made - in UNIX seconds UTC
    pub invested: u64,
    // when this investment can be claimed - in UNIX seconds UTC
    pub maturity_date: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const LOCATIONS: Map<&str, Location> = Map::new("locations");
// TODO: don't use Vec
pub const INVESTMENTS: Map<(&Addr, &str), Vec<Investment>> = Map::new("investments");
