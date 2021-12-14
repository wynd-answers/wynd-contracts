use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal};
use cw_storage_plus::Item;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub oracles: Vec<Addr>,
    // how many oracles must approve to be valid
    pub required_weight: u64,
    // how many seconds between job creation and getting all results
    pub max_waiting_period: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Measurement {
    /// when this actual measurement was taken
    /// most measurements will be slightly different times
    pub time: u64,
    /// location as R3 index
    pub hex: R3,
    /// value (accepts "123.456" type numbers, with 18 digits before, 18 after the decimal place)
    pub val: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct JobResult {
    pub metric: String,
    pub measurements: Vec<Measurement>,
    // when this was tallied (unix time UTC in sec)
    pub tally_time: u64,
}

// TODO: add some helper methods to work with R3 data
// TODO: remove pub from u64 and use accessors
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct R3(pub u64);
