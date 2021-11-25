#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    ensure_eq, from_slice, to_binary, Addr, Binary, Deps, DepsMut, Env, Event, MessageInfo, Order,
    Response, StdError, StdResult, Uint128,
};
use cw2::set_contract_version;
use cw20::{Cw20CoinVerified, Cw20Contract, Cw20ExecuteMsg, Cw20ReceiveMsg};

use crate::error::ContractError;
use crate::msg::{
    ConfigResponse, ExecuteMsg, InfoResponse, InstantiateMsg, InvestmentResponse,
    ListInvestmentsResponse, OracleValues, QueryMsg, ReceiveMsg,
};
use crate::r3::validate_r3;
use crate::state::{Config, Investment, Location, Measurement, CONFIG, INVESTMENTS, LOCATIONS};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:wynd-invest";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        oracle: deps.api.addr_validate(&msg.oracle)?,
        token: deps.api.addr_validate(&msg.token)?,
        max_investment_hex: msg.max_investment_hex,
        maturity_days: msg.maturity_days,
        measurement_window: msg.measurement_window,
    };
    CONFIG.save(deps.storage, &config)?;

    let empty_hex = Location::default();
    for index in msg.locations.into_iter() {
        let hex = validate_r3(index)?;
        LOCATIONS.save(deps.storage, &hex, &empty_hex)?;
    }

    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => receive(deps, env, info, msg),
        ExecuteMsg::Withdraw {} => withdraw(deps, env, info),
        ExecuteMsg::StoreOracle { values } => store_oracle(deps, env, info, values),
    }
}

pub fn receive(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    wrapper: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let coin = Cw20CoinVerified {
        address: info.sender,
        amount: wrapper.amount,
    };
    let sender = deps.api.addr_validate(&wrapper.sender)?;
    let msg: ReceiveMsg = from_slice(&wrapper.msg)?;

    match msg {
        ReceiveMsg::Invest { hex } => invest(deps, env, sender, coin, hex),
    }
}

pub fn invest(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    coin: Cw20CoinVerified,
    hex: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.token != coin.address {
        return Err(ContractError::InvalidToken(coin.address.into()));
    }

    let hex = validate_r3(hex)?;
    let invested = env.block.time.seconds();
    let maturity_date = invested + config.maturity_days * 86400;

    // update investment info in Location
    let mut location = LOCATIONS.load(deps.storage, &hex)?;
    location.add_investment(coin.amount);
    LOCATIONS.save(deps.storage, &hex, &location)?;

    let last_index = location.cur_index.ok_or(ContractError::NoDataPresent)?;
    if last_index.time < env.block.time.seconds() - config.measurement_window * 86400 {
        return Err(ContractError::DataTooOld {
            days: config.measurement_window,
        });
    }

    let invest = Investment {
        amount: coin.amount,
        baseline_index: last_index.value,
        invested_time: invested,
        maturity_time: maturity_date,
    };
    INVESTMENTS.update::<_, StdError>(deps.storage, (&sender, &hex), |invs| {
        let mut invs = invs.unwrap_or_default();
        invs.push(invest);
        Ok(invs)
    })?;

    let evt = Event::new("invest")
        .add_attribute("index", hex)
        .add_attribute("amount", coin.amount.to_string())
        .add_attribute("investor", sender);
    Ok(Response::new().add_event(evt))
}

pub fn withdraw(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;

    let investments: StdResult<Vec<_>> = INVESTMENTS
        .prefix_de(&info.sender)
        .range(deps.storage, None, None, Order::Ascending)
        .collect();

    let mut to_withdraw = Uint128::zero();

    for (hex, invests) in investments?.into_iter() {
        let mut loc = LOCATIONS.load(deps.storage, &hex)?;

        // this filters out to leave non-mature investments
        // returns a tally from all mature investments of original amounts and rewards to pay out
        let init: Result<_, ContractError> = Ok((
            Vec::with_capacity(invests.len()),
            Uint128::zero(),
            Uint128::zero(),
        ));
        let (invests, reward, orig) = invests.into_iter().fold(init, |acc, invest| {
            let (mut v, total, orig) = acc?;
            match invest.reward(&env, &loc, &cfg) {
                Some(reward) => Ok((v, total + reward, orig + invest.amount)),
                None => {
                    v.push(invest);
                    Ok((v, total, orig))
                }
            }
        })?;
        // update location state with the redeemed investments
        loc.finish_investment(orig)?;
        // and tally up how much to pay out
        to_withdraw += reward;

        LOCATIONS.save(deps.storage, &hex, &loc)?;
        INVESTMENTS.save(deps.storage, (&info.sender, &hex), &invests)?;
    }

    if to_withdraw.is_zero() {
        return Ok(Response::new());
    }

    let msg = Cw20Contract(cfg.token).call(Cw20ExecuteMsg::Transfer {
        recipient: info.sender.to_string(),
        amount: to_withdraw,
    })?;
    let evt = Event::new("withdraw")
        .add_attribute("amount", to_withdraw.to_string())
        .add_attribute("investor", info.sender);
    Ok(Response::new().add_event(evt).add_message(msg))
}

pub fn store_oracle(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    values: Vec<OracleValues>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    ensure_eq!(config.oracle, info.sender, ContractError::Unauthorized {});

    let count = values.len();
    for val in values.into_iter() {
        let hex = validate_r3(val.index)?;
        let mut loc = LOCATIONS
            .load(deps.storage, &hex)
            .map_err(|_| ContractError::UnregisteredLocation(hex.clone()))?;
        if val.time > env.block.time.seconds() {
            return Err(ContractError::OracleFromTheFuture(val.time));
        }
        loc.cur_index = Some(Measurement {
            value: val.value,
            time: val.time,
        });
        LOCATIONS.save(deps.storage, &hex, &loc)?;
    }

    let evt = Event::new("oracle").add_attribute("count", count.to_string());
    Ok(Response::new().add_event(evt))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::Config {} => Ok(to_binary(&query_config(deps)?)?),
        QueryMsg::Info { hex } => Ok(to_binary(&query_info(deps, hex)?)?),
        QueryMsg::ListInvestments { investor, hex } => {
            Ok(to_binary(&list_investments(deps, investor, hex)?)?)
        }
    }
}

fn query_config(deps: Deps) -> Result<ConfigResponse, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    Ok(config)
}

fn query_info(deps: Deps, hex: String) -> Result<InfoResponse, ContractError> {
    let hex = validate_r3(hex)?;
    let info = LOCATIONS.load(deps.storage, &hex)?;
    Ok(InfoResponse {
        cur_index: info.cur_index,
        total_invested: info.total_invested,
        current_invested: info.current_invested,
        total_investments: info.total_investments,
        current_investments: info.current_investments,
    })
}

fn list_investments(
    deps: Deps,
    investor: String,
    hex: Option<String>,
) -> Result<ListInvestmentsResponse, ContractError> {
    let hex = hex.map(validate_r3).transpose()?;
    let investor = deps.api.addr_validate(&investor)?;

    let investments = if let Some(hex) = hex {
        INVESTMENTS
            .load(deps.storage, (&investor, &hex))?
            .into_iter()
            .map(|inv| InvestmentResponse::new(inv, hex.clone()))
            .collect()
    } else {
        // all for this investor
        let nested: StdResult<Vec<Vec<_>>> = INVESTMENTS
            .prefix_de(&investor)
            .range(deps.storage, None, None, Order::Ascending)
            .map(|res| {
                let (hex, invs) = res?;
                Ok(invs
                    .into_iter()
                    .map(|i| InvestmentResponse::new(i, hex.clone()))
                    .collect())
            })
            .collect();
        nested?.into_iter().flatten().collect()
    };

    Ok(ListInvestmentsResponse { investments })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{Decimal, MemoryStorage, Storage, SubMsg, WasmMsg};

    fn env_at(secs: u64) -> Env {
        let mut env = mock_env();
        env.block.time = env.block.time.plus_seconds(secs);
        env
    }

    fn time_at(secs: u64) -> u64 {
        mock_env().block.time.seconds() + secs
    }

    fn init_with_locations(locs: &[&str]) -> InstantiateMsg {
        InstantiateMsg {
            oracle: "oracle".to_string(),
            locations: locs.iter().map(|s| s.to_string()).collect(),
            token: "token".to_string(),
            max_investment_hex: Uint128::new(12345678),
            maturity_days: 28,
            measurement_window: 7,
        }
    }

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let info = mock_info("creator", &[]);
        let msg = init_with_locations(&["8765437FFFFFFFF", "1284639ffffffff"]);

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg.clone()).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query_config(deps.as_ref()).unwrap();
        let expected = Config {
            oracle: Addr::unchecked(msg.oracle),
            token: Addr::unchecked(msg.token),
            max_investment_hex: msg.max_investment_hex,
            maturity_days: msg.maturity_days,
            measurement_window: msg.measurement_window,
        };
        assert_eq!(res, expected);

        // check out the locations
        let info1 = query_info(deps.as_ref(), "8765437FFFFFfff".into()).unwrap();
        assert_eq!(info1, InfoResponse::default());
        let info2 = query_info(deps.as_ref(), "1284639ffFFffff".into()).unwrap();
        assert_eq!(info2, InfoResponse::default());
    }

    #[test]
    fn validate_locations_in_init() {
        let mut deps = mock_dependencies();

        let info = mock_info("creator", &[]);
        let msg = init_with_locations(&["foobar"]);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    }

    #[test]
    fn set_oracle_data() {
        let mut deps = mock_dependencies();

        let location = "8362718ffffffff";
        let info = mock_info("creator", &[]);
        let msg = init_with_locations(&[location]);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = query_info(deps.as_ref(), location.into()).unwrap();
        assert_eq!(info, InfoResponse::default());

        // set this with some oracle data
        let msg = ExecuteMsg::StoreOracle {
            values: vec![OracleValues {
                index: location.to_string(),
                value: Decimal::percent(1234),
                time: time_at(20),
            }],
        };

        // fail if in the future
        let err = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("oracle", &[]),
            msg.clone(),
        )
        .unwrap_err();
        assert_eq!(err, ContractError::OracleFromTheFuture(time_at(20)));

        // fail if not oracle
        let err = execute(
            deps.as_mut(),
            env_at(1234),
            mock_info("token", &[]),
            msg.clone(),
        )
        .unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});

        // just right
        execute(deps.as_mut(), env_at(1234), mock_info("oracle", &[]), msg).unwrap();

        // check updated
        let info = query_info(deps.as_ref(), location.into()).unwrap();
        let mut expected = InfoResponse::new();
        expected.cur_index = Some(Measurement {
            value: Decimal::percent(1234),
            time: time_at(20),
        });
        assert_eq!(info, expected);

        // reject bad location
        let msg = ExecuteMsg::StoreOracle {
            values: vec![OracleValues {
                index: "9362718FFffffff".to_string(),
                value: Decimal::percent(1234),
                time: time_at(20),
            }],
        };
        let err = execute(deps.as_mut(), env_at(1234), mock_info("oracle", &[]), msg).unwrap_err();
        assert_eq!(
            err,
            ContractError::UnregisteredLocation("9362718ffffffff".to_string())
        );
    }

    #[test]
    fn check_investment() {
        let mut deps = mock_dependencies();

        let location = "8362718ffffffff";
        let info = mock_info("creator", &[]);
        let msg = init_with_locations(&[location]);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = query_info(deps.as_ref(), location.into()).unwrap();
        assert_eq!(info, InfoResponse::default());

        // cannot invest without data
        let amount = Uint128::new(777000);
        let payload = ReceiveMsg::Invest {
            hex: location.to_string(),
        };
        let wrapped = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: "investor".to_string(),
            amount,
            msg: to_binary(&payload).unwrap(),
        });

        let err = execute(
            deps.as_mut(),
            env_at(1234),
            mock_info("token", &[]),
            wrapped.clone(),
        )
        .unwrap_err();
        assert_eq!(err, ContractError::NoDataPresent);

        // set this with some oracle data
        let measurement = Measurement {
            value: Decimal::percent(1234),
            time: time_at(20),
        };
        let oracle = ExecuteMsg::StoreOracle {
            values: vec![OracleValues {
                index: location.to_string(),
                value: measurement.value,
                time: measurement.time,
            }],
        };
        execute(
            deps.as_mut(),
            env_at(1234),
            mock_info("oracle", &[]),
            oracle,
        )
        .unwrap();

        // try to invest again
        execute(
            deps.as_mut(),
            env_at(5000),
            mock_info("token", &[]),
            wrapped,
        )
        .unwrap();

        // check investment
        let mut invests =
            list_investments(deps.as_ref(), "investor".into(), Some(location.into())).unwrap();
        let invests2 = list_investments(deps.as_ref(), "investor".into(), None).unwrap();
        assert_eq!(invests, invests2);
        assert_eq!(invests.investments.len(), 1);
        let invest = invests.investments.pop().unwrap();
        let expected = InvestmentResponse {
            hex: location.to_string(),
            amount,
            baseline_index: measurement.value,
            invested: time_at(5000),
            maturity_date: time_at(5000 + 28 * 86400),
        };
        assert_eq!(invest, expected);

        // update oracle
        let value2 = Decimal::percent(4321);
        let oracle = ExecuteMsg::StoreOracle {
            values: vec![OracleValues {
                index: location.to_string(),
                value: value2,
                time: time_at(86400 + 5000),
            }],
        };
        execute(
            deps.as_mut(),
            env_at(86400 + 10000),
            mock_info("oracle", &[]),
            oracle,
        )
        .unwrap();

        // invest again
        let amount2 = Uint128::new(12345678);
        let payload = ReceiveMsg::Invest {
            hex: location.to_string(),
        };
        let wrapped = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: "investor".to_string(),
            amount: amount2,
            msg: to_binary(&payload).unwrap(),
        });
        execute(
            deps.as_mut(),
            env_at(2 * 86400),
            mock_info("token", &[]),
            wrapped,
        )
        .unwrap();

        // get both
        let expected2 = InvestmentResponse {
            hex: location.to_string(),
            amount: amount2,
            baseline_index: value2,
            invested: time_at(2 * 86400),
            maturity_date: time_at(30 * 86400),
        };
        let invests = list_investments(deps.as_ref(), "investor".into(), None).unwrap();
        assert_eq!(invests.investments.len(), 2);
        assert_eq!(invests.investments, vec![expected, expected2]);
    }

    #[test]
    fn withdraw_happy_path() {
        let mut deps = mock_dependencies();

        let location = "8362718ffffffff";
        let location2 = "9362718ffffffff";
        let info = mock_info("creator", &[]);
        let msg = init_with_locations(&[location, location2]);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // oracle info for one
        let oracle = ExecuteMsg::StoreOracle {
            values: vec![OracleValues {
                index: location.to_string(),
                value: Decimal::percent(1234),
                time: time_at(200),
            }],
        };
        execute(
            deps.as_mut(),
            env_at(86400),
            mock_info("oracle", &[]),
            oracle,
        )
        .unwrap();

        // invest there
        let amount = Uint128::new(808000);
        let payload = ReceiveMsg::Invest {
            hex: location.to_string(),
        };
        let wrapped = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: "investor".to_string(),
            amount,
            msg: to_binary(&payload).unwrap(),
        });
        execute(
            deps.as_mut(),
            env_at(2 * 86400),
            mock_info("token", &[]),
            wrapped,
        )
        .unwrap();

        // more oracle data
        let oracle = ExecuteMsg::StoreOracle {
            values: vec![OracleValues {
                index: location.to_string(),
                value: Decimal::percent(2468),
                time: time_at(86400 * 31),
            }],
        };
        execute(
            deps.as_mut(),
            env_at(86400 * 31 + 2000),
            mock_info("oracle", &[]),
            oracle,
        )
        .unwrap();

        // now withdrawl works
        let withdraw = ExecuteMsg::Withdraw {};
        let res = execute(
            deps.as_mut(),
            env_at(35 * 86400),
            mock_info("investor", &[]),
            withdraw,
        )
        .unwrap();

        // value doubled, we get 50% out
        let end_amount = amount * Decimal::percent(50);
        let expected = Cw20ExecuteMsg::Transfer {
            recipient: "investor".to_string(),
            amount: end_amount,
        };
        assert_eq!(
            res.messages,
            vec![SubMsg::new(WasmMsg::Execute {
                contract_addr: "token".to_string(),
                msg: to_binary(&expected).unwrap(),
                funds: vec![]
            })]
        );
    }

    #[test]
    fn withdraw_error_cases() {
        let mut deps = mock_dependencies();

        let location = "8362718ffffffff";
        let location2 = "9362718ffffffff";
        let info = mock_info("creator", &[]);
        let msg = init_with_locations(&[location, location2]);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // oracle info for one
        let oracle = ExecuteMsg::StoreOracle {
            values: vec![OracleValues {
                index: location.to_string(),
                value: Decimal::percent(1234),
                time: time_at(200),
            }],
        };
        execute(
            deps.as_mut(),
            env_at(86400),
            mock_info("oracle", &[]),
            oracle,
        )
        .unwrap();

        // invest there
        let amount = Uint128::new(808000);
        let payload = ReceiveMsg::Invest {
            hex: location.to_string(),
        };
        let wrapped = ExecuteMsg::Receive(Cw20ReceiveMsg {
            sender: "investor".to_string(),
            amount,
            msg: to_binary(&payload).unwrap(),
        });
        execute(
            deps.as_mut(),
            env_at(2 * 86400),
            mock_info("token", &[]),
            wrapped,
        )
        .unwrap();

        // we have to copy the storage to "revert" all changes in the failed execution for
        // properly handling multiple runs
        let orig_storage = clone_storage(&deps.storage);

        // withdraw too early, no op
        let withdraw = ExecuteMsg::Withdraw {};
        let res = execute(
            deps.as_mut(),
            env_at(22 * 86400),
            mock_info("investor", &[]),
            withdraw,
        )
        .unwrap();
        assert_eq!(res.messages, vec![]);

        // withdraw later, no data, no op
        deps.storage = clone_storage(&orig_storage);
        let withdraw = ExecuteMsg::Withdraw {};
        let res = execute(
            deps.as_mut(),
            env_at(35 * 86400),
            mock_info("investor", &[]),
            withdraw,
        )
        .unwrap();
        assert_eq!(res.messages, vec![]);
    }

    fn clone_storage(orig: &MemoryStorage) -> MemoryStorage {
        let mut new_store = MemoryStorage::new();
        for (k, v) in orig.range(None, None, Order::Ascending) {
            new_store.set(&k, &v);
        }
        new_store
    }
}
