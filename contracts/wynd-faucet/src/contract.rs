#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, WasmMsg,
};
use cw2::set_contract_version;
use cw20::{BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};

use crate::error::ContractError;
use crate::msg::{CallsResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, CALLS, CONFIG};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:wynd-faucet";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let config = Config {
        token: deps.api.addr_validate(&msg.token)?,
        amount: msg.amount,
        max_requests: msg.max_requests.unwrap_or(1),
    };

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::RequestFunds {} => request_funds(deps, info),
    }
}

pub fn request_funds(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    CALLS.update(deps.storage, &info.sender, |count| {
        let count = count.unwrap_or(0);
        if count >= config.max_requests {
            Err(ContractError::UsedAllCalls(count))
        } else {
            Ok(count + 1)
        }
    })?;

    let msg = Cw20ExecuteMsg::Transfer {
        recipient: info.sender.to_string(),
        amount: config.amount,
    };
    let res = Response::new()
        .add_message(WasmMsg::Execute {
            contract_addr: config.token.into(),
            msg: to_binary(&msg)?,
            funds: vec![],
        })
        .add_attribute("fund", info.sender)
        .add_attribute("amount", config.amount.to_string());

    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Balance {} => to_binary(&query_balance(deps, env)?),
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::Calls { address } => to_binary(&query_calls(deps, address)?),
    }
}

fn query_balance(deps: Deps, env: Env) -> StdResult<BalanceResponse> {
    let config = CONFIG.load(deps.storage)?;
    let query = Cw20QueryMsg::Balance {
        address: env.contract.address.into(),
    };
    deps.querier.query_wasm_smart(config.token, &query)
}

fn query_config(deps: Deps) -> StdResult<Config> {
    let config = CONFIG.load(deps.storage)?;
    Ok(config)
}

fn query_calls(deps: Deps, account: String) -> StdResult<CallsResponse> {
    let account = deps.api.addr_validate(&account)?;
    let calls = CALLS.load(deps.storage, &account)?;
    Ok(CallsResponse { calls })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{Addr, SubMsg, Uint128};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();

        let amount = Uint128::new(123456789);
        let msg = InstantiateMsg {
            token: "wynd".to_string(),
            amount,
            max_requests: None,
        };
        let info = mock_info("creator", &[]);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // it worked, let's query the state
        let res = query_config(deps.as_ref()).unwrap();
        let expected = Config {
            token: Addr::unchecked("wynd"),
            amount,
            max_requests: 1,
        };
        assert_eq!(res, expected);
    }

    #[test]
    fn fund_once() {
        let mut deps = mock_dependencies();

        let amount = Uint128::new(123456789);
        let msg = InstantiateMsg {
            token: "wynd".to_string(),
            amount,
            max_requests: None,
        };
        let info = mock_info("creator", &[]);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // anyone can use it... once
        let info = mock_info("anyone", &[]);
        let msg = ExecuteMsg::RequestFunds {};
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        let msg = cw20::Cw20ExecuteMsg::Transfer {
            recipient: "anyone".to_string(),
            amount,
        };
        assert_eq!(
            res.messages,
            [SubMsg::new(WasmMsg::Execute {
                contract_addr: "wynd".to_string(),
                msg: to_binary(&msg).unwrap(),
                funds: vec![]
            })]
        );

        // cannot call a second time
        let info = mock_info("anyone", &[]);
        let msg = ExecuteMsg::RequestFunds {};
        let err = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
        assert_eq!(err, ContractError::UsedAllCalls(1));

        // different user can use it
        let info = mock_info("elsewhere", &[]);
        let msg = ExecuteMsg::RequestFunds {};
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
        let msg = cw20::Cw20ExecuteMsg::Transfer {
            recipient: "elsewhere".to_string(),
            amount,
        };
        assert_eq!(
            res.messages,
            [SubMsg::new(WasmMsg::Execute {
                contract_addr: "wynd".to_string(),
                msg: to_binary(&msg).unwrap(),
                funds: vec![]
            })]
        );
    }
}
