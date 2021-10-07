use cosmwasm_std::{
    entry_point, to_binary, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Reply,
    Response, StdError, StdResult,
};

use cw721::Expiration;

use crate::state::{State, STATE};
use andromeda_protocol::{
    modules::address_list::{on_address_list_reply, REPLY_ADDRESS_LIST},
    modules::{hooks::MessageHooks, Module},
    require::require,
    timelock::{
        get_funds, hold_funds, release_funds, Escrow, ExecuteMsg, GetLockedFundsResponse,
        GetTimelockConfigResponse, InstantiateMsg, QueryMsg,
    },
};

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let state = State {
        owner: info.sender.clone(),
        address_list: msg.address_list.clone(),
    };

    let mut res = Response::default();

    if msg.address_list.is_some() {
        let addr_res =
            msg.address_list
                .clone()
                .unwrap()
                .on_instantiate(&deps, info.clone(), env.clone())?;
        res = res.add_submessages(addr_res.msgs);
    }

    STATE.save(deps.storage, &state)?;
    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {
    if msg.result.is_err() {
        return Err(StdError::generic_err(msg.result.unwrap_err()));
    }

    match msg.id {
        REPLY_ADDRESS_LIST => on_address_list_reply(deps, msg),
        _ => Err(StdError::generic_err("reply id is invalid")),
    }
}

#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    let state = STATE.load(deps.storage)?;

    if state.address_list.is_some() {
        let addr_list = state.address_list.unwrap();
        addr_list.on_execute(&deps, info.clone(), env.clone())?;
    }

    match msg {
        ExecuteMsg::HoldFunds {
            expiration,
            recipient,
        } => execute_hold_funds(deps, info, expiration, recipient),
        ExecuteMsg::ReleaseFunds {} => execute_release_funds(deps, env, info),
    }
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetLockedFunds { address } => to_binary(&query_held_funds(deps, address)?),
        QueryMsg::GetTimelockConfig {} => to_binary(&query_config(deps)?),
    }
}

fn query_held_funds(deps: Deps, address: String) -> StdResult<GetLockedFundsResponse> {
    let hold_funds = get_funds(deps.storage, address.clone())?;
    Ok(GetLockedFundsResponse { funds: hold_funds })
}

fn query_config(deps: Deps) -> StdResult<GetTimelockConfigResponse> {
    let state = STATE.load(deps.storage)?;

    let address_list_contract = match state.address_list.clone() {
        None => None,
        Some(addr_list) => addr_list.get_contract_address(deps.storage),
    };

    Ok(GetTimelockConfigResponse {
        address_list: state.address_list,
        address_list_contract,
    })
}

fn execute_hold_funds(
    deps: DepsMut,
    info: MessageInfo,
    expiration: Expiration,
    recipient: Option<String>,
) -> StdResult<Response> {
    let result: Option<Escrow> = get_funds(deps.storage, info.sender.to_string())?;
    require(
        result.is_none(),
        StdError::generic_err("Funds are already being held for this address"),
    )?;

    let sent_funds: Vec<Coin> = info.funds.clone();
    if sent_funds.len() == 0 {
        return Err(StdError::generic_err("No funds provided"));
    }

    let rec = recipient.unwrap_or(info.sender.to_string());

    let escrow = Escrow {
        coins: sent_funds,
        expiration,
        recipient: rec,
    };
    hold_funds(escrow, deps.storage, info.sender.to_string())?;

    Ok(Response::default())
}

fn execute_release_funds(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let result: Option<Escrow> = get_funds(deps.storage, info.sender.to_string())?; // StdResult<Option<HoldFunds>>

    if result.is_none() {
        return Err(StdError::generic_err("No locked funds for your address"));
    }

    let funds: Escrow = result.unwrap();

    match funds.expiration {
        Expiration::AtTime(t) => {
            if t > env.block.time {
                return Err(StdError::generic_err("Your funds are still locked"));
            }
        }
        Expiration::AtHeight(h) => {
            if h > env.block.height {
                return Err(StdError::generic_err("Your funds are still locked"));
            }
        }
        Expiration::Never {} => {}
    }

    release_funds(deps.storage, info.sender.to_string());
    Ok(Response::new().add_message(BankMsg::Send {
        to_address: funds.recipient,
        amount: funds.coins,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::{
        from_binary,
        testing::{mock_dependencies, mock_env, mock_info},
        Addr,
    };

    fn mock_state() -> State {
        State {
            owner: Addr::unchecked("owner"),
            address_list: None,
        }
    }

    #[test]
    fn test_instantiate() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let owner = "owner";
        let info = mock_info(owner, &[]);
        let msg = InstantiateMsg { address_list: None };
        let res = instantiate(deps.as_mut(), env, info.clone(), msg).unwrap();

        assert_eq!(0, res.messages.len());

        //checking
        let state = STATE.load(deps.as_ref().storage).unwrap();
        assert_eq!(state.owner, owner);
    }

    #[test]
    fn test_execute_hold_funds() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let owner = "owner";
        let funds = vec![Coin::new(1000, "uusd")];
        let expiration = Expiration::Never {};
        let info = mock_info(owner, &funds.clone());
        STATE.save(deps.as_mut().storage, &mock_state()).unwrap();

        let msg = ExecuteMsg::HoldFunds {
            expiration: Expiration::Never {},
            recipient: None,
        };

        //add address for registered moderator

        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
        assert_eq!(Response::default(), res);

        let query_msg = QueryMsg::GetLockedFunds {
            address: owner.to_string(),
        };

        let res = query(deps.as_ref(), env.clone(), query_msg).unwrap();
        let val: GetLockedFundsResponse = from_binary(&res).unwrap();
        let expected = Escrow {
            coins: funds.clone(),
            expiration: expiration.clone(),
            recipient: owner.to_string(),
        };

        assert_eq!(val.funds.unwrap(), expected);
    }

    #[test]
    fn test_execute_release_funds() {
        let mut deps = mock_dependencies(&[]);
        let env = mock_env();
        let owner = "owner";
        let funds = vec![Coin::new(1000, "uusd")];
        STATE.save(deps.as_mut().storage, &mock_state()).unwrap();

        let info = mock_info(owner, &funds.clone());
        let msg = ExecuteMsg::HoldFunds {
            expiration: Expiration::Never {},
            recipient: None,
        };

        //add address for registered moderator
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();

        let info = mock_info(owner, &[]);
        let msg = ExecuteMsg::ReleaseFunds {};
        let res = execute(deps.as_mut(), env.clone(), info.clone(), msg.clone()).unwrap();
        let bank_msg = BankMsg::Send {
            to_address: owner.to_string(),
            amount: funds.clone(),
        };

        let expected = Response::default().add_message(bank_msg);

        assert_eq!(res, expected);
    }
}
