use cosmwasm_std::Uint128;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// which cw20 token we send
    pub token: String,
    /// how much we send on request
    pub amount: Uint128,
    /// max times we pay out for a given account, default 1
    pub max_requests: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// this will pay out some tokens to the caller, or return error if they already used their share
    RequestFunds {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// How many tokens this has left. returns cw20::BalanceResponse
    Balance {},
    /// Read the Config of the contract. returns Config
    Config {},
    /// How many times the given address has used the faucet. returns CallsResponse
    Calls { address: String },
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CallsResponse {
    pub calls: u32,
}
