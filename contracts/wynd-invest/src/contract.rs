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

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use cosmwasm_std::testing::{
//         mock_dependencies, mock_dependencies_with_balance, mock_env, mock_info,
//     };
//     use cosmwasm_std::{coins, from_binary};
//
//     #[test]
//     fn proper_initialization() {
//         let mut deps = mock_dependencies();
//
//         let msg = InstantiateMsg { count: 17 };
//         let info = mock_info("creator", &coins(1000, "earth"));
//
//         // we can just call .unwrap() to assert this was a success
//         let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
//         assert_eq!(0, res.messages.len());
//
//         // it worked, let's query the state
//         let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
//         let value: CountResponse = from_binary(&res).unwrap();
//         assert_eq!(17, value.count);
//     }
//
//     #[test]
//     fn increment() {
//         let mut deps = mock_dependencies_with_balance(&coins(2, "token"));
//
//         let msg = InstantiateMsg { count: 17 };
//         let info = mock_info("creator", &coins(2, "token"));
//         let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
//
//         // beneficiary can release it
//         let info = mock_info("anyone", &coins(2, "token"));
//         let msg = ExecuteMsg::Increment {};
//         let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
//
//         // should increase counter by 1
//         let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
//         let value: CountResponse = from_binary(&res).unwrap();
//         assert_eq!(18, value.count);
//     }
//
//     #[test]
//     fn reset() {
//         let mut deps = mock_dependencies_with_balance(&coins(2, "token"));
//
//         let msg = InstantiateMsg { count: 17 };
//         let info = mock_info("creator", &coins(2, "token"));
//         let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
//
//         // beneficiary can release it
//         let unauth_info = mock_info("anyone", &coins(2, "token"));
//         let msg = ExecuteMsg::Reset { count: 5 };
//         let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
//         match res {
//             Err(ContractError::Unauthorized {}) => {}
//             _ => panic!("Must return unauthorized error"),
//         }
//
//         // only the original creator can reset the counter
//         let auth_info = mock_info("creator", &coins(2, "token"));
//         let msg = ExecuteMsg::Reset { count: 5 };
//         let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();
//
//         // should now be 5
//         let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
//         let value: CountResponse = from_binary(&res).unwrap();
//         assert_eq!(5, value.count);
//     }
// }
