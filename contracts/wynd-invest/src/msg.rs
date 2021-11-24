use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{Config, Investment, Measurement};
use cosmwasm_std::{Decimal, Uint128};
use cw20::Cw20ReceiveMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    // address of oracle contract (this allows writing data)
    pub oracle: String,
    // list of all r3 locations that can be invested (as hex)
    pub locations: Vec<String>,
    // address of the cw20 token that we use for payment
    pub token: String,
    // maximum amount that can be invested in one hex
    pub max_investment_hex: Uint128,
    // how many days the investment takes until maturity (eg. we pay out in the results in 30 days, 180 days)
    pub maturity_days: u64,
    // how many days margin we have from measurement to usage.
    // when investing, the latest data must be within X days
    // when investment finishes, there must be data within X days of maturity
    pub measurement_window: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    // this will return funds from all finished investments
    Withdraw {},
    StoreOracle { values: Vec<OracleValues> },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct OracleValues {
    // r3 index of the measurement
    pub index: String,
    // value measured
    pub value: Decimal,
    // unix time (UTC) in seconds
    pub time: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveMsg {
    // returns investment_id in event and
    Invest { hex: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    // Information about one hex spot - current oracle, investment counts
    Info {
        hex: String,
    },
    // List all investments by user, possibly filtering on one hex location
    // FIXME: add pagination?
    ListInvestments {
        investor: String,
        hex: Option<String>,
    },
}

pub type ConfigResponse = Config;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct InfoResponse {
    pub cur_index: Option<Measurement>,
    // amount of money invested here
    pub total_invested: Uint128,
    pub current_invested: Uint128,
    // number of individual investments made (people)
    pub total_investments: u64,
    pub current_investments: u64,
}

impl InfoResponse {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ListInvestmentsResponse {
    pub investments: Vec<InvestmentResponse>,
}

// Note: we do not include address here. It is verbose and implied in the query
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InvestmentResponse {
    pub hex: String,
    // how much was invested
    pub amount: Uint128,
    // starting value when investment was created
    pub baseline_index: Decimal,
    // when this investment was made - in UNIX seconds UTC
    pub invested: u64,
    // when this investment can be claimed - in UNIX seconds UTC
    pub maturity_date: u64,
}

impl InvestmentResponse {
    pub fn new(invest: Investment, hex: String) -> Self {
        InvestmentResponse {
            hex,
            amount: invest.amount,
            baseline_index: invest.baseline_index,
            invested: invest.invested_time,
            maturity_date: invest.maturity_time,
        }
    }
}
