use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ContractError;
use cosmwasm_std::{Addr, Decimal, Env, Fraction, Uint128};
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
    // how many days margin we have from measurement to usage.
    // when investing, the latest data must be within X days
    // when investment finishes, there must be data within X days of maturity
    pub measurement_window: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct Location {
    pub cur_index: Option<Measurement>,
    // amount of money invested here
    pub total_invested: Uint128,
    pub current_invested: Uint128,
    // number of individual investments made (people)
    pub total_investments: u64,
    pub current_investments: u64,
}

impl Location {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_investment(&mut self, amount: Uint128) {
        self.total_invested += amount;
        self.current_invested += amount;
        self.total_investments += 1;
        self.current_investments += 1;
    }

    pub fn finish_investment(&mut self, amount: Uint128, count: u64) -> Result<(), ContractError> {
        self.current_investments -= count;
        self.current_invested = self.current_invested.checked_sub(amount)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, Default)]
pub struct Measurement {
    pub value: Decimal,
    // unix time (UTC) in seconds
    pub time: u64,
}

impl Measurement {
    pub fn new(value: Decimal, time: u64) -> Measurement {
        Measurement { value, time }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Investment {
    // how much was invested
    pub amount: Uint128,
    // starting value when investment was created
    pub baseline_index: Decimal,
    // when this investment was made - in UNIX seconds UTC
    pub invested_time: u64,
    // when this investment can be claimed - in UNIX seconds UTC
    pub maturity_time: u64,
}

impl Investment {
    /// whether or not this investment has reached maturity date and can be withdrawn
    pub fn is_mature(&self, env: &Env) -> bool {
        env.block.time.seconds() >= self.maturity_time
    }

    /// calculates the reward. if it is not mature, or there is insufficient data
    /// to provide a result, then it will return None
    pub fn reward(&self, env: &Env, loc: &Location, cfg: &Config) -> Option<Uint128> {
        if !self.is_mature(env) {
            return None;
        }
        // TODO: we need to store historical data... you cannot just wait it out
        if let Some(measure) = &loc.cur_index {
            match measure.time.checked_sub(self.maturity_time) {
                Some(val) if val <= cfg.measurement_window * 86400 => {
                    // measurement after maturity, within window
                    // calculate ratio, positive, if measurement below baseline
                    // no code to divide Decimals, so we do this
                    let ratio = Decimal::from_ratio(
                        self.baseline_index.numerator(),
                        measure.value.numerator(),
                    );
                    let reward = self.amount * ratio;
                    Some(reward)
                }
                Some(_) => {
                    // measurement after maturity, after window, return 100%
                    Some(self.amount)
                }
                None => {
                    // measurement before maturity date
                    None
                }
            }
        } else {
            None
        }
    }
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const LOCATIONS: Map<&str, Location> = Map::new("locations");
pub const INVESTMENTS: Map<(&Addr, &str), Vec<Investment>> = Map::new("investments");

#[cfg(test)]
mod test {
    use super::*;
    use cosmwasm_std::testing::mock_env;

    fn env_at(secs: u64) -> Env {
        let mut env = mock_env();
        env.block.time = env.block.time.plus_seconds(secs);
        env
    }

    fn time_at(secs: u64) -> u64 {
        mock_env().block.time.seconds() + secs
    }

    fn loc_with_measurement(measure: Measurement) -> Location {
        let mut loc = Location::new();
        loc.cur_index = Some(measure);
        loc
    }

    #[test]
    fn investment_rewards() {
        let maturity_time = time_at(123 + 7 * 86400);
        let cfg = Config {
            oracle: Addr::unchecked(""),
            token: Addr::unchecked(""),
            max_investment_hex: Uint128::new(1234567890123),
            maturity_days: 7,
            measurement_window: 2,
        };
        let invest = Investment {
            amount: Uint128::new(10000),
            baseline_index: Decimal::percent(450), // 4.5
            invested_time: time_at(123),
            maturity_time,
        };

        // should get 1.5x payout
        let result = Decimal::percent(300);
        let no_measure = Location::default();
        let old_measurement = loc_with_measurement(Measurement::new(result, maturity_time - 1000));
        let good_measurement =
            loc_with_measurement(Measurement::new(result, maturity_time + 86400));
        let late_measurement =
            loc_with_measurement(Measurement::new(result, maturity_time + 3 * 86400));

        // env correct but no measurement
        let env = env_at(maturity_time + 2);
        assert!(invest.reward(&env, &no_measure, &cfg).is_none());

        // env correct but old measurement
        assert!(invest.reward(&env, &old_measurement, &cfg).is_none());

        // env correct and good measurement -> 1.5x payout
        assert_eq!(
            invest.reward(&env, &good_measurement, &cfg),
            Some(Uint128::new(15000))
        );

        // env correct and late measurement -> 100% payout
        assert_eq!(
            invest.reward(&env, &late_measurement, &cfg),
            Some(Uint128::new(10000))
        );

        // measurement good, not yet mature, no payout (not sure how this happens...)
        let env = env_at(0);
        assert!(invest.reward(&env, &good_measurement, &cfg).is_none());
    }
}
