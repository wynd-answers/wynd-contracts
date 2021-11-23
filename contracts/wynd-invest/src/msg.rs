use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Decimal, Timestamp, Uint128};
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
    // TODO: time periods - average over X days, how many days til maturity?
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    // this will return funds from all finished investments
    Withdraw {},
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
    // Information about one hex spot - current oracle, investment counts
    Info { hex: String },
    // List all investments by user, possibly filtering on one hex location
    // FIXME: add pagination?
    ListInvestments { addr: String, hex: Option<String> },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InfoResponse {
    pub cur_index: Decimal,
    // amount of money invested here
    pub total_invested: Uint128,
    pub current_invested: Uint128,
    // number of individual investments made (people)
    pub total_investments: u64,
    pub current_investments: u64,
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
    pub baseline_index: Decimal,
    // when this investment was made - in UNIX seconds UTC
    pub invested: u64,
    // when this investment can be claimed - in UNIX seconds UTC
    pub maturity_date: u64,
}
