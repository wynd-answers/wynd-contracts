use crate::state::{Measurement, R3};
use cosmwasm_std::{Addr, Decimal};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub oracles: Vec<Addr>,
    // how many oracles must approve to be valid
    pub required_weight: u64,
    // how many seconds between job creation and getting all results
    pub max_waiting_period: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    // This creates a new job from the data and returns the ID JSON-encoded in Response.data,
    // so the caller can query it later
    CreateJob {
        // approx time we want data for in seconds
        data_time: u64,
        // the metric we are interested in
        metric: String,
        // R3 indexes to provide data for
        hexes: Vec<R3>,
        // R3 level to use (0-15)
        resolution: u32,
    },

    // called by an oracle once it has processed the work
    SubmitResult {
        job: u64,
        // all use the same metric requested
        measurements: Vec<Measurement>,
    },

    // called by anyone once enough data is there
    TallyResult {
        job: u64,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // returns info on the job, as well as how many votes are ready / state
    QueryJob {
        id: u64,
    },
    // return jobs needing results, to be called by oracles
    ListPendingJobs {
        start_after: Option<u64>,
        limit: Option<u32>,
    },
}
