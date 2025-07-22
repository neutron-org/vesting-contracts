use crate::asset::{native_asset_info, token_asset_info};
use crate::builder::VestingBaseBuilder;
use crate::error::{ext_unsupported_err, ContractError};
use crate::handlers::{execute, query};
use crate::msg::{
    Cw20HookMsg, ExecuteMsg, ExecuteMsgManaged, ExecuteMsgWithManagers, QueryMsg,
    QueryMsgHistorical, QueryMsgWithManagers,
};
use crate::types::{
    Config, Extensions, OrderBy, VestingAccount, VestingAccountResponse, VestingAccountsResponse,
    VestingSchedule, VestingSchedulePoint, VestingState,
};
use cosmwasm_std::testing::{
    message_info, mock_dependencies, mock_env, MockApi, MockQuerier, MockStorage,
};
use cosmwasm_std::{
    from_json, to_json_binary, Addr, BankMsg, Coin, CosmosMsg, Env, OwnedDeps, StdError, Uint128,
    WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

type MockDeps = OwnedDeps<MockStorage, MockApi, MockQuerier>;

fn setup_contract() -> (MockDeps, Env, Addr, Addr) {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let owner = deps.api.addr_make("owner");
    let token_info_manager = deps.api.addr_make("token_info_manager");

    VestingBaseBuilder::default()
        .build(
            deps.as_mut(),
            owner.to_string(),
            token_info_manager.to_string(),
        )
        .unwrap();

    (deps, env, owner, token_info_manager)
}

fn setup_contract_with_token() -> (MockDeps, Env, Addr, Addr, Addr) {
    let (mut deps, env, owner, token_info_manager) = setup_contract();
    let vesting_token = deps.api.addr_make("vesting_token");

    VestingBaseBuilder::default()
        .historical()
        .managed()
        .build(
            deps.as_mut(),
            owner.to_string(),
            token_info_manager.to_string(),
        )
        .unwrap();

    let info = message_info(&token_info_manager, &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::SetVestingToken {
            vesting_token: token_asset_info(vesting_token.clone()),
        },
    )
        .unwrap();

    (deps, env, owner, token_info_manager, vesting_token)
}

fn setup_contract_with_extensions() -> (MockDeps, Env, Addr, Addr, Vec<String>) {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let owner = deps.api.addr_make("owner");
    let token_info_manager = deps.api.addr_make("token_info_manager");
    let vesting_managers = vec![
        deps.api.addr_make("manager1").into_string(),
        deps.api.addr_make("manager2").into_string(),
    ];

    VestingBaseBuilder::default()
        .historical()
        .managed()
        .with_managers(vesting_managers.clone())
        .build(
            deps.as_mut(),
            owner.to_string(),
            token_info_manager.to_string(),
        )
        .unwrap();

    (deps, env, owner, token_info_manager, vesting_managers)
}

fn create_vesting_schedule(
    start_time: u64,
    start_amount: Uint128,
    end_time: Option<u64>,
    end_amount: Option<Uint128>,
) -> VestingSchedule {
    VestingSchedule {
        start_point: VestingSchedulePoint {
            time: start_time,
            amount: start_amount,
        },
        end_point: if let (Some(time), Some(amount)) = (end_time, end_amount) {
            Some(VestingSchedulePoint { time, amount })
        } else {
            None
        },
        disabled: false,
    }
}

// ========== BUILDER TESTS ==========

#[test]
fn test_proper_building_standard() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");
    let token_info_manager = deps.api.addr_make("token_info_manager");
    let env = mock_env();
    let info = message_info(&owner, &[]);
    VestingBaseBuilder::default()
        .build(
            deps.as_mut(),
            owner.to_string(),
            token_info_manager.to_string(),
        )
        .unwrap();

    // check initialization
    assert_eq!(
        from_json::<Config>(&query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap())
            .unwrap(),
        Config {
            owner: owner.clone(),
            token_info_manager: token_info_manager.clone(),
            vesting_token: None,
            extensions: Extensions {
                historical: false,
                managed: false,
                with_managers: false
            }
        }
    );

    // make sure with_managers extension is not enabled
    assert_eq!(
        query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::WithManagersExtension {
                msg: QueryMsgWithManagers::VestingManagers {}
            }
        )
            .unwrap_err(),
        ext_unsupported_err("with_managers")
    );

    // make sure historical extension is not enabled
    assert_eq!(
        query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::HistoricalExtension {
                msg: QueryMsgHistorical::UnclaimedTotalAmountAtHeight { height: 1000u64 }
            }
        )
            .unwrap_err(),
        ext_unsupported_err("historical")
    );

    // make sure managed extension is not enabled
    assert_eq!(
        execute(
            deps.as_mut(),
            env,
            info,
            ExecuteMsg::ManagedExtension {
                msg: ExecuteMsgManaged::RemoveVestingAccounts {
                    vesting_accounts: vec![],
                    clawback_account: String::from("clawback")
                }
            },
        )
            .unwrap_err(),
        ext_unsupported_err("managed").into()
    );
}

#[test]
fn test_proper_building_managers() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");
    let token_info_manager = deps.api.addr_make("token_info_manager");
    let env = mock_env();
    let info = message_info(&owner, &[]);
    let vesting_managers = vec![
        deps.api.addr_make("manager1").into_string(),
        deps.api.addr_make("manager2").into_string(),
    ];
    VestingBaseBuilder::default()
        .with_managers(vesting_managers.clone())
        .build(
            deps.as_mut(),
            owner.to_string(),
            token_info_manager.to_string(),
        )
        .unwrap();

    // check initialization
    assert_eq!(
        from_json::<Config>(&query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap())
            .unwrap(),
        Config {
            owner: owner.clone(),
            token_info_manager: token_info_manager.clone(),
            vesting_token: None,
            extensions: Extensions {
                historical: false,
                managed: false,
                with_managers: true
            }
        }
    );

    // make sure with_managers extension is enabled
    let managers_addrs = from_json::<Vec<Addr>>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::WithManagersExtension {
                msg: QueryMsgWithManagers::VestingManagers {},
            },
        )
            .unwrap(),
    )
        .unwrap();
    let managers_strings: Vec<String> = managers_addrs.iter().map(|a| a.to_string()).collect();
    assert_eq!(managers_strings, vesting_managers);

    // make sure historical extension is not enabled
    assert_eq!(
        query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::HistoricalExtension {
                msg: QueryMsgHistorical::UnclaimedTotalAmountAtHeight { height: 1000u64 }
            }
        )
            .unwrap_err(),
        ext_unsupported_err("historical")
    );

    // make sure managed extension is not enabled
    assert_eq!(
        execute(
            deps.as_mut(),
            env,
            info,
            ExecuteMsg::ManagedExtension {
                msg: ExecuteMsgManaged::RemoveVestingAccounts {
                    vesting_accounts: vec![],
                    clawback_account: String::from("clawback"),
                },
            },
        )
            .unwrap_err(),
        ext_unsupported_err("managed").into()
    );
}

#[test]
fn test_proper_building_historical() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");
    let token_info_manager = deps.api.addr_make("token_info_manager");
    let env = mock_env();
    let info = message_info(&owner, &[]);
    VestingBaseBuilder::default()
        .historical()
        .build(
            deps.as_mut(),
            owner.to_string(),
            token_info_manager.to_string(),
        )
        .unwrap();

    // check initialization
    assert_eq!(
        from_json::<Config>(&query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap())
            .unwrap(),
        Config {
            owner: owner.clone(),
            token_info_manager: token_info_manager.clone(),
            vesting_token: None,
            extensions: Extensions {
                historical: true,
                managed: false,
                with_managers: false
            }
        }
    );

    // make sure with_managers extension is not enabled
    assert_eq!(
        query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::WithManagersExtension {
                msg: QueryMsgWithManagers::VestingManagers {}
            }
        )
            .unwrap_err(),
        ext_unsupported_err("with_managers")
    );

    // make sure historical extension is enabled
    query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::HistoricalExtension {
            msg: QueryMsgHistorical::UnclaimedTotalAmountAtHeight { height: 1000u64 },
        },
    )
        .unwrap();

    // make sure managed extension is not enabled
    assert_eq!(
        execute(
            deps.as_mut(),
            env,
            info,
            ExecuteMsg::ManagedExtension {
                msg: ExecuteMsgManaged::RemoveVestingAccounts {
                    vesting_accounts: vec![],
                    clawback_account: String::from("clawback")
                }
            },
        )
            .unwrap_err(),
        ext_unsupported_err("managed").into()
    );
}

#[test]
fn test_proper_building_managed() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");
    let token_info_manager = deps.api.addr_make("token_info_manager");
    let env = mock_env();
    VestingBaseBuilder::default()
        .managed()
        .build(
            deps.as_mut(),
            owner.to_string(),
            token_info_manager.to_string(),
        )
        .unwrap();

    // check initialization and set vesting token
    assert_eq!(
        from_json::<Config>(&query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap())
            .unwrap(),
        Config {
            owner: owner.clone(),
            token_info_manager: token_info_manager.clone(),
            vesting_token: None,
            extensions: Extensions {
                historical: false,
                managed: true,
                with_managers: false
            }
        }
    );

    let info = message_info(&token_info_manager, &[]);
    let ntrn_token = deps.api.addr_make("ntrn_token");
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::SetVestingToken {
            vesting_token: token_asset_info(ntrn_token),
        },
    )
        .unwrap();

    // make sure with_managers extension is not enabled
    assert_eq!(
        query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::WithManagersExtension {
                msg: QueryMsgWithManagers::VestingManagers {}
            }
        )
            .unwrap_err(),
        ext_unsupported_err("with_managers")
    );

    // make sure historical extension is not enabled
    assert_eq!(
        query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::HistoricalExtension {
                msg: QueryMsgHistorical::UnclaimedTotalAmountAtHeight { height: 1000u64 }
            }
        )
            .unwrap_err(),
        ext_unsupported_err("historical")
    );

    let info = message_info(&owner, &[]);
    let clawback = deps.api.addr_make("ntrn_token").into_string();
    // make sure managed extension is enabled
    execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::ManagedExtension {
            msg: ExecuteMsgManaged::RemoveVestingAccounts {
                vesting_accounts: vec![],
                clawback_account: clawback,
            },
        },
    )
        .unwrap();
}

#[test]
fn test_proper_building_all_extensions() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");
    let token_info_manager = deps.api.addr_make("token_info_manager");
    let env = mock_env();
    let vesting_managers = vec![
        deps.api.addr_make("manager1").into_string(),
        deps.api.addr_make("manager2").into_string(),
    ];
    VestingBaseBuilder::default()
        .historical()
        .managed()
        .with_managers(vesting_managers.clone())
        .build(
            deps.as_mut(),
            owner.to_string(),
            token_info_manager.to_string(),
        )
        .unwrap();

    // check initialization and set vesting token
    assert_eq!(
        from_json::<Config>(&query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap())
            .unwrap(),
        Config {
            owner: owner.clone(),
            token_info_manager: token_info_manager.clone(),
            vesting_token: None,
            extensions: Extensions {
                historical: true,
                managed: true,
                with_managers: true
            }
        }
    );

    let info = message_info(&token_info_manager, &[]);
    let ntrn_token = deps.api.addr_make("ntrn_token");
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::SetVestingToken {
            vesting_token: token_asset_info(ntrn_token),
        },
    )
        .unwrap();

    // make sure with_managers extension is enabled
    let managers_addrs = from_json::<Vec<Addr>>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::WithManagersExtension {
                msg: QueryMsgWithManagers::VestingManagers {},
            },
        )
            .unwrap(),
    )
        .unwrap();
    let managers_strings: Vec<String> = managers_addrs.iter().map(|a| a.to_string()).collect();
    assert_eq!(managers_strings, vesting_managers);

    // make sure historical extension is enabled
    query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::HistoricalExtension {
            msg: QueryMsgHistorical::UnclaimedTotalAmountAtHeight { height: 1000u64 },
        },
    )
        .unwrap();

    // make sure managed extension is enabled
    let info = message_info(&owner, &[]);
    let clawback = deps.api.addr_make("ntrn_token").into_string();
    execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::ManagedExtension {
            msg: ExecuteMsgManaged::RemoveVestingAccounts {
                vesting_accounts: vec![],
                clawback_account: clawback,
            },
        },
    )
        .unwrap();
}

// ========== EXECUTE MESSAGE TESTS ==========

#[test]
fn test_set_vesting_token() {
    let mut deps = mock_dependencies();
    let owner = deps.api.addr_make("owner");
    let token_info_manager = deps.api.addr_make("token_info_manager");
    let env = mock_env();
    VestingBaseBuilder::default()
        .build(
            deps.as_mut(),
            owner.to_string(),
            token_info_manager.to_string(),
        )
        .unwrap();

    // check initialization
    assert_eq!(
        from_json::<Config>(&query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap())
            .unwrap(),
        Config {
            owner: owner.clone(),
            token_info_manager: token_info_manager.clone(),
            vesting_token: None,
            extensions: Extensions {
                historical: false,
                managed: false,
                with_managers: false
            }
        }
    );

    let info = message_info(&deps.api.addr_make("stranger"), &[]);
    let ntrn_token = deps.api.addr_make("ntrn_token");
    // set vesting token by a stranger -> Unauthorized
    assert_eq!(
        execute(
            deps.as_mut(),
            env.clone(),
            info,
            ExecuteMsg::SetVestingToken {
                vesting_token: token_asset_info(ntrn_token.clone()),
            },
        )
        .unwrap_err(),
        ContractError::Unauthorized {},
    );

    // set vesting token by the manager -> Success
    let info = message_info(&token_info_manager, &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::SetVestingToken {
            vesting_token: token_asset_info(ntrn_token.clone()),
        },
    )
        .unwrap();

    assert_eq!(
        from_json::<Config>(&query(deps.as_ref(), env.clone(), QueryMsg::Config {}).unwrap())
            .unwrap(),
        Config {
            owner: owner.clone(),
            token_info_manager: token_info_manager.clone(),
            vesting_token: Some(token_asset_info(ntrn_token.clone())),
            extensions: Extensions {
                historical: false,
                managed: false,
                with_managers: false
            }
        }
    );

    let info = message_info(&owner, &[]);
    let not_ntrn_token = deps.api.addr_make("not_ntrn_token");
    // set vesting token second time by the owner -> VestingTokenAlreadySet
    assert_eq!(
        execute(
            deps.as_mut(),
            env.clone(),
            info,
            ExecuteMsg::SetVestingToken {
                vesting_token: token_asset_info(not_ntrn_token),
            },
        )
        .unwrap_err(),
        ContractError::VestingTokenAlreadySet {},
    );

    assert_eq!(
        from_json::<Config>(&query(deps.as_ref(), env, QueryMsg::Config {}).unwrap()).unwrap(),
        Config {
            owner,
            token_info_manager,
            vesting_token: Some(token_asset_info(ntrn_token)),
            extensions: Extensions {
                historical: false,
                managed: false,
                with_managers: false
            }
        }
    );
}

#[test]
fn test_set_vesting_token_by_owner() {
    let (mut deps, env, owner, _) = setup_contract();
    let vesting_token = deps.api.addr_make("vesting_token");

    // Owner can set vesting token
    let info = message_info(&owner, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::SetVestingToken {
            vesting_token: token_asset_info(vesting_token.clone()),
        },
    )
        .unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "set_vesting_token");
    assert_eq!(res.attributes[1].key, "vesting_token");
    assert_eq!(res.attributes[1].value, vesting_token.to_string());

    // Verify token is set
    let config =
        from_json::<Config>(&query(deps.as_ref(), env, QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!(config.vesting_token, Some(token_asset_info(vesting_token)));
}

#[test]
fn test_set_vesting_token_native() {
    let (mut deps, env, owner, _) = setup_contract();
    let native_denom = "uatom";

    // Set native token as vesting token
    let info = message_info(&owner, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::SetVestingToken {
            vesting_token: native_asset_info(native_denom.to_string()),
        },
    )
        .unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "set_vesting_token");
    assert_eq!(res.attributes[1].key, "vesting_token");
    assert_eq!(res.attributes[1].value, native_denom);

    // Verify token is set
    let config =
        from_json::<Config>(&query(deps.as_ref(), env, QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!(
        config.vesting_token,
        Some(native_asset_info(native_denom.to_string()))
    );
}

#[test]
fn test_register_vesting_accounts_cw20() {
    let (mut deps, env, owner, _, vesting_token) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");
    let user2 = deps.api.addr_make("user2");
    let amount = Uint128::new(1000);

    let vesting_accounts = vec![
        VestingAccount {
            address: user1.to_string(),
            schedules: vec![create_vesting_schedule(
                env.block.time.seconds(),
                Uint128::new(500),
                None,
                None,
            )],
        },
        VestingAccount {
            address: user2.to_string(),
            schedules: vec![create_vesting_schedule(
                env.block.time.seconds(),
                Uint128::new(500),
                None,
                None,
            )],
        },
    ];

    // Register vesting accounts via CW20 receive
    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount,
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Receive(cw20_msg),
    )
        .unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "register_vesting_accounts");
    assert_eq!(res.attributes[1].key, "deposited");
    assert_eq!(res.attributes[1].value, amount.to_string());

    // Verify vesting accounts are registered
    let user1_account = from_json::<VestingAccountResponse>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::VestingAccount {
                address: user1.to_string(),
            },
        )
            .unwrap(),
    )
        .unwrap();
    assert_eq!(user1_account.address, user1);
    assert_eq!(user1_account.info.schedules.len(), 1);

    let user2_account = from_json::<VestingAccountResponse>(
        &query(
            deps.as_ref(),
            env,
            QueryMsg::VestingAccount {
                address: user2.to_string(),
            },
        )
            .unwrap(),
    )
        .unwrap();
    assert_eq!(user2_account.address, user2);
    assert_eq!(user2_account.info.schedules.len(), 1);
}

#[test]
fn test_register_vesting_accounts_native() {
    let (mut deps, env, owner, _) = setup_contract();
    let native_denom = "uatom";

    // Set native token as vesting token
    let info = message_info(&owner, &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::SetVestingToken {
            vesting_token: native_asset_info(native_denom.to_string()),
        },
    )
        .unwrap();

    let user1 = deps.api.addr_make("user1");
    let amount = Uint128::new(1000);

    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds(),
            amount,
            None,
            None,
        )],
    }];

    // Register vesting accounts with native token payment
    let info = message_info(
        &owner,
        &[Coin {
            denom: native_denom.to_string(),
            amount,
        }],
    );
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::RegisterVestingAccounts { vesting_accounts },
    )
        .unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "register_vesting_accounts");
    assert_eq!(res.attributes[1].key, "deposited");
    assert_eq!(res.attributes[1].value, amount.to_string());

    // Verify vesting account is registered
    let user1_account = from_json::<VestingAccountResponse>(
        &query(
            deps.as_ref(),
            env,
            QueryMsg::VestingAccount {
                address: user1.to_string(),
            },
        )
            .unwrap(),
    )
        .unwrap();
    assert_eq!(user1_account.address, user1);
    assert_eq!(user1_account.info.schedules.len(), 1);
}

#[test]
fn test_register_vesting_accounts_unauthorized() {
    let (mut deps, env, _, _, vesting_token) = setup_contract_with_token();

    let stranger = deps.api.addr_make("stranger");
    let user1 = deps.api.addr_make("user1");
    let amount = Uint128::new(1000);

    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds(),
            amount,
            None,
            None,
        )],
    }];

    // Try to register from unauthorized sender
    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: stranger.to_string(),
        amount,
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    let err = execute(deps.as_mut(), env, info, ExecuteMsg::Receive(cw20_msg)).unwrap_err();

    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_register_vesting_accounts_amount_mismatch() {
    let (mut deps, env, owner, _, vesting_token) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");
    let sent_amount = Uint128::new(1000);
    let schedule_amount = Uint128::new(500); // Different from sent amount

    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds(),
            schedule_amount,
            None,
            None,
        )],
    }];

    // Register with mismatched amounts
    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount: sent_amount,
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    let err = execute(deps.as_mut(), env, info, ExecuteMsg::Receive(cw20_msg)).unwrap_err();

    assert_eq!(err, ContractError::VestingScheduleAmountError {});
}

#[test]
fn test_register_vesting_accounts_invalid_schedule() {
    let (mut deps, env, owner, _, vesting_token) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");
    let amount = Uint128::new(1000);

    // Invalid schedule: start_time >= end_time
    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds() + 100,
            Uint128::new(500),
            Some(env.block.time.seconds()),
            Some(amount),
        )],
    }];

    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount,
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    let err = execute(deps.as_mut(), env, info, ExecuteMsg::Receive(cw20_msg)).unwrap_err();

    assert_eq!(err, ContractError::VestingScheduleError(user1.to_string()));
}

#[test]
fn test_claim_tokens_cw20() {
    let (mut deps, mut env, owner, _, vesting_token) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");
    let amount = Uint128::new(1000);

    // Register vesting account
    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds(),
            Uint128::new(500),
            Some(env.block.time.seconds() + 100),
            Some(amount),
        )],
    }];

    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount,
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Receive(cw20_msg),
    )
        .unwrap();

    // Move time forward to make some tokens available
    env.block.time = env.block.time.plus_seconds(50);

    // Claim tokens
    let info = message_info(&user1, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Claim {
            recipient: None,
            amount: None,
        },
    )
        .unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "claim");
    assert_eq!(res.attributes[1].key, "address");
    assert_eq!(res.attributes[1].value, user1.to_string());

    // Check that transfer message is created
    assert_eq!(res.messages.len(), 1);
    match &res.messages[0].msg {
        CosmosMsg::Wasm(WasmMsg::Execute {
                            contract_addr, msg, ..
                        }) => {
            assert_eq!(contract_addr, &vesting_token.to_string());
            let transfer_msg: Cw20ExecuteMsg = from_json(msg).unwrap();
            match transfer_msg {
                Cw20ExecuteMsg::Transfer { recipient, amount } => {
                    assert_eq!(recipient, user1.to_string());
                    assert!(amount > Uint128::zero());
                }
                _ => panic!("Expected Transfer message"),
            }
        }
        _ => panic!("Expected Wasm message"),
    }
}

#[test]
fn test_claim_tokens_native() {
    let (mut deps, env, owner, _) = setup_contract();
    let native_denom = "uatom";

    // Set native token as vesting token
    let info = message_info(&owner, &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::SetVestingToken {
            vesting_token: native_asset_info(native_denom.to_string()),
        },
    )
        .unwrap();

    let user1 = deps.api.addr_make("user1");
    let amount = Uint128::new(1000);

    // Register vesting account
    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds(),
            amount,
            None,
            None,
        )],
    }];

    let info = message_info(
        &owner,
        &[Coin {
            denom: native_denom.to_string(),
            amount,
        }],
    );
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::RegisterVestingAccounts { vesting_accounts },
    )
        .unwrap();

    // Claim tokens
    let info = message_info(&user1, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Claim {
            recipient: None,
            amount: None,
        },
    )
        .unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "claim");
    assert_eq!(res.attributes[1].key, "address");
    assert_eq!(res.attributes[1].value, user1.to_string());

    // Check that bank send message is created
    assert_eq!(res.messages.len(), 1);
    match &res.messages[0].msg {
        CosmosMsg::Bank(BankMsg::Send {
                            to_address,
                            amount: coins,
                        }) => {
            assert_eq!(to_address, &user1.to_string());
            assert_eq!(coins.len(), 1);
            assert_eq!(coins[0].denom, native_denom);
            assert_eq!(coins[0].amount, amount);
        }
        _ => panic!("Expected Bank message"),
    }
}

#[test]
fn test_claim_tokens_with_recipient() {
    let (mut deps, env, owner, _, vesting_token) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");
    let recipient = deps.api.addr_make("recipient");
    let amount = Uint128::new(1000);

    // Register vesting account
    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds(),
            amount,
            None,
            None,
        )],
    }];

    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount,
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Receive(cw20_msg),
    )
        .unwrap();

    // Claim tokens with specific recipient
    let info = message_info(&user1, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Claim {
            recipient: Some(recipient.to_string()),
            amount: None,
        },
    )
        .unwrap();

    // Check that transfer message goes to recipient
    assert_eq!(res.messages.len(), 1);
    match &res.messages[0].msg {
        CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
            let transfer_msg: Cw20ExecuteMsg = from_json(msg).unwrap();
            match transfer_msg {
                Cw20ExecuteMsg::Transfer {
                    recipient: msg_recipient,
                    ..
                } => {
                    assert_eq!(msg_recipient, recipient.to_string());
                }
                _ => panic!("Expected Transfer message"),
            }
        }
        _ => panic!("Expected Wasm message"),
    }
}

#[test]
fn test_claim_tokens_partial_amount() {
    let (mut deps, env, owner, _, vesting_token) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");
    let total_amount = Uint128::new(1000);
    let claim_amount = Uint128::new(500);

    // Register vesting account
    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds(),
            total_amount,
            None,
            None,
        )],
    }];

    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount: total_amount,
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Receive(cw20_msg),
    )
        .unwrap();

    // Claim partial amount
    let info = message_info(&user1, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Claim {
            recipient: None,
            amount: Some(claim_amount),
        },
    )
        .unwrap();

    // Check that transfer message has correct amount
    assert_eq!(res.messages.len(), 1);
    match &res.messages[0].msg {
        CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
            let transfer_msg: Cw20ExecuteMsg = from_json(msg).unwrap();
            match transfer_msg {
                Cw20ExecuteMsg::Transfer { amount, .. } => {
                    assert_eq!(amount, claim_amount);
                }
                _ => panic!("Expected Transfer message"),
            }
        }
        _ => panic!("Expected Wasm message"),
    }
}

#[test]
fn test_claim_tokens_insufficient_amount() {
    let (mut deps, env, owner, _, vesting_token) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");
    let total_amount = Uint128::new(1000);
    let claim_amount = Uint128::new(1500); // More than available

    // Register vesting account
    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds(),
            total_amount,
            None,
            None,
        )],
    }];

    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount: total_amount,
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Receive(cw20_msg),
    )
        .unwrap();

    // Try to claim more than available
    let info = message_info(&user1, &[]);
    let err = execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::Claim {
            recipient: None,
            amount: Some(claim_amount),
        },
    )
        .unwrap_err();

    assert_eq!(err, ContractError::AmountIsNotAvailable {});
}

#[test]
fn test_claim_tokens_no_vesting_account() {
    let (mut deps, env, _, _, _) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");

    // Try to claim without having a vesting account
    let info = message_info(&user1, &[]);
    let err = execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::Claim {
            recipient: None,
            amount: None,
        },
    )
        .unwrap_err();

    // Should get a storage error since the account doesn't exist
    assert!(matches!(err, ContractError::Std(_)));
}

#[test]
fn test_force_claim_tokens() {
    let (mut deps, mut env, owner, _, vesting_token) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");
    let amount = Uint128::new(1000);

    // Register vesting account with future vesting
    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds(),
            Uint128::new(0),
            Some(env.block.time.seconds() + 1000),
            Some(amount),
        )],
    }];

    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount,
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Receive(cw20_msg),
    )
        .unwrap();

    // Move time forward a bit but not to full vesting
    env.block.time = env.block.time.plus_seconds(200);

    // Force claim tokens
    let info = message_info(&user1, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        ExecuteMsg::ForceClaim { recipient: None },
    )
        .unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "force_claim");
    assert_eq!(res.attributes[1].key, "address");
    assert_eq!(res.attributes[1].value, user1.to_string());

    // Check that some amount was claimed (should be more than normal vesting due to force claim)
    assert_eq!(res.messages.len(), 1);
    match &res.messages[0].msg {
        CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
            let transfer_msg: Cw20ExecuteMsg = from_json(msg).unwrap();
            match transfer_msg {
                Cw20ExecuteMsg::Transfer { amount, .. } => {
                    assert_eq!(amount, Uint128::new(600)); // 200 + 800 / 2 = 600 (available to claim + 50% of the remaining)
                }
                _ => panic!("Expected Transfer message"),
            }
        }
        _ => panic!("Expected Wasm message"),
    }

    // User hasn't anything to claim anymore
    let available = from_json::<Uint128>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::AvailableAmount {
                address: user1.to_string(),
            },
        )
            .unwrap(),
    )
        .unwrap();
    assert_eq!(available, Uint128::zero());

    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Claim {
            recipient: None,
            amount: None,
        },
    )
        .unwrap();
    assert_eq!(res.messages.len(), 0);
    assert_eq!(res.attributes[2].value, "0");
    assert_eq!(res.attributes[3].value, "0");

    // Owner can get remove vesting accounts and get unclaimed amount
    let clawback_account = deps.api.addr_make("clawback");
    let info = message_info(&owner, &[]);
    let res = execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::ManagedExtension {
            msg: ExecuteMsgManaged::RemoveVestingAccounts {
                vesting_accounts: vec![user1.to_string()],
                clawback_account: clawback_account.to_string(),
            },
        },
    )
        .unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "remove_vesting_accounts");
    assert_eq!(res.messages.len(), 1);
    match &res.messages[0].msg {
        CosmosMsg::Wasm(WasmMsg::Execute { msg, .. }) => {
            let transfer_msg: Cw20ExecuteMsg = from_json(msg).unwrap();
            match transfer_msg {
                Cw20ExecuteMsg::Transfer { amount, recipient } => {
                    assert_eq!(amount, Uint128::new(400)); // 400 - the remaining of the users vesting
                    assert_eq!(recipient, clawback_account.to_string());
                }
                _ => panic!("Expected Transfer message"),
            }
        }
        _ => panic!("Expected Wasm message"),
    }
}

#[test]
fn test_ownership_proposal() {
    let (mut deps, env, owner, _) = setup_contract();
    let new_owner = deps.api.addr_make("new_owner");
    let expires_in = 86400u64; // 1 day

    // Propose new owner
    let info = message_info(&owner, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::ProposeNewOwner {
            owner: new_owner.to_string(),
            expires_in,
        },
    )
        .unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "propose_new_owner");
    assert_eq!(res.attributes[1].key, "new_owner");
    assert_eq!(res.attributes[1].value, new_owner.to_string());

    // Claim ownership
    let info = message_info(&new_owner, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::ClaimOwnership {},
    )
        .unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "claim_ownership");
    assert_eq!(res.attributes[1].key, "new_owner");
    assert_eq!(res.attributes[1].value, new_owner.to_string());

    // Verify ownership changed
    let config =
        from_json::<Config>(&query(deps.as_ref(), env, QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!(config.owner, new_owner);
}

#[test]
fn test_ownership_proposal_unauthorized() {
    let (mut deps, env, _, _) = setup_contract();
    let stranger = deps.api.addr_make("stranger");
    let new_owner = deps.api.addr_make("new_owner");

    // Try to propose new owner as stranger
    let info = message_info(&stranger, &[]);
    let err = execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::ProposeNewOwner {
            owner: new_owner.to_string(),
            expires_in: 86400u64,
        },
    )
        .unwrap_err();

    assert!(matches!(err, ContractError::Std(_)));
}

#[test]
fn test_ownership_proposal_expired() {
    let (mut deps, mut env, owner, _) = setup_contract();
    let new_owner = deps.api.addr_make("new_owner");
    let expires_in = 100u64;

    // Propose new owner
    let info = message_info(&owner, &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::ProposeNewOwner {
            owner: new_owner.to_string(),
            expires_in,
        },
    )
        .unwrap();

    // Move time past expiration
    env.block.time = env.block.time.plus_seconds(expires_in + 1);

    // Try to claim expired ownership
    let info = message_info(&new_owner, &[]);
    let err = execute(deps.as_mut(), env, info, ExecuteMsg::ClaimOwnership {}).unwrap_err();

    assert!(matches!(err, ContractError::Std(_)));
}

#[test]
fn test_drop_ownership_proposal() {
    let (mut deps, env, owner, _) = setup_contract();
    let new_owner = deps.api.addr_make("new_owner");

    // Propose new owner
    let info = message_info(&owner, &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::ProposeNewOwner {
            owner: new_owner.to_string(),
            expires_in: 86400u64,
        },
    )
        .unwrap();

    // Drop ownership proposal
    let info = message_info(&owner, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::DropOwnershipProposal {},
    )
        .unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "drop_ownership_proposal");

    // Try to claim ownership after dropping proposal
    let info = message_info(&new_owner, &[]);
    let err = execute(deps.as_mut(), env, info, ExecuteMsg::ClaimOwnership {}).unwrap_err();

    assert!(matches!(err, ContractError::Std(_)));
}

// ========== QUERY MESSAGE TESTS ==========

#[test]
fn test_query_config() {
    let (deps, env, owner, token_info_manager) = setup_contract();

    let config =
        from_json::<Config>(&query(deps.as_ref(), env, QueryMsg::Config {}).unwrap()).unwrap();

    assert_eq!(config.owner, owner);
    assert_eq!(config.token_info_manager, token_info_manager);
    assert_eq!(config.vesting_token, None);
    assert!(!config.extensions.historical);
    assert!(!config.extensions.managed);
    assert!(!config.extensions.with_managers);
}

#[test]
fn test_query_vesting_account() {
    let (mut deps, env, owner, _, vesting_token) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");
    let amount = Uint128::new(1000);

    // Register vesting account
    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds(),
            amount,
            None,
            None,
        )],
    }];

    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount,
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Receive(cw20_msg),
    )
        .unwrap();

    // Query vesting account
    let account = from_json::<VestingAccountResponse>(
        &query(
            deps.as_ref(),
            env,
            QueryMsg::VestingAccount {
                address: user1.to_string(),
            },
        )
            .unwrap(),
    )
        .unwrap();

    assert_eq!(account.address, user1);
    assert_eq!(account.info.schedules.len(), 1);
    assert_eq!(account.info.released_amount, Uint128::zero());
    assert_eq!(account.info.schedules[0].start_point.amount, amount);
}

#[test]
fn test_query_vesting_account_not_found() {
    let (deps, env, _, _, _) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");

    // Query non-existent vesting account
    let err = query(
        deps.as_ref(),
        env,
        QueryMsg::VestingAccount {
            address: user1.to_string(),
        },
    )
        .unwrap_err();

    assert!(matches!(err, StdError::NotFound { .. }));
}

#[test]
fn test_query_vesting_accounts() {
    let (mut deps, env, owner, _, vesting_token) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");
    let user2 = deps.api.addr_make("user2");
    let amount = Uint128::new(1000);

    // Register multiple vesting accounts
    let vesting_accounts = vec![
        VestingAccount {
            address: user1.to_string(),
            schedules: vec![create_vesting_schedule(
                env.block.time.seconds(),
                Uint128::new(500),
                None,
                None,
            )],
        },
        VestingAccount {
            address: user2.to_string(),
            schedules: vec![create_vesting_schedule(
                env.block.time.seconds(),
                Uint128::new(500),
                None,
                None,
            )],
        },
    ];

    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount,
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Receive(cw20_msg),
    )
        .unwrap();

    // Query all vesting accounts
    let accounts = from_json::<VestingAccountsResponse>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::VestingAccounts {
                start_after: None,
                limit: None,
                order_by: None,
            },
        )
            .unwrap(),
    )
        .unwrap();

    assert_eq!(accounts.vesting_accounts.len(), 2);

    // Query with limit
    let accounts = from_json::<VestingAccountsResponse>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::VestingAccounts {
                start_after: None,
                limit: Some(1),
                order_by: None,
            },
        )
            .unwrap(),
    )
        .unwrap();

    assert_eq!(accounts.vesting_accounts.len(), 1);

    // Query with start_after
    let accounts = from_json::<VestingAccountsResponse>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::VestingAccounts {
                start_after: Some(user1.to_string()),
                limit: None,
                order_by: Some(OrderBy::Asc),
            },
        )
            .unwrap(),
    )
        .unwrap();

    assert_eq!(accounts.vesting_accounts.len(), 1);
    assert_eq!(accounts.vesting_accounts[0].address, user2);
}

#[test]
fn test_query_available_amount() {
    let (mut deps, mut env, owner, _, vesting_token) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");
    let amount = Uint128::new(1000);

    // Register vesting account with linear vesting
    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds(),
            Uint128::new(500),
            Some(env.block.time.seconds() + 100),
            Some(amount),
        )],
    }];

    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount,
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Receive(cw20_msg),
    )
        .unwrap();

    // Query available amount at start
    let available = from_json::<Uint128>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::AvailableAmount {
                address: user1.to_string(),
            },
        )
            .unwrap(),
    )
        .unwrap();
    assert_eq!(available, Uint128::new(500)); // Start amount

    // Move time forward halfway
    env.block.time = env.block.time.plus_seconds(50);

    let available = from_json::<Uint128>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::AvailableAmount {
                address: user1.to_string(),
            },
        )
            .unwrap(),
    )
        .unwrap();
    assert_eq!(available, Uint128::new(750)); // Start + half of linear vesting

    // Move time to end
    env.block.time = env.block.time.plus_seconds(50);

    let available = from_json::<Uint128>(
        &query(
            deps.as_ref(),
            env,
            QueryMsg::AvailableAmount {
                address: user1.to_string(),
            },
        )
            .unwrap(),
    )
        .unwrap();
    assert_eq!(available, amount); // Full amount
}

#[test]
fn test_query_vesting_state() {
    let (mut deps, env, owner, _, vesting_token) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");
    let amount = Uint128::new(1000);

    // Register vesting account
    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds(),
            amount,
            None,
            None,
        )],
    }];

    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount,
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Receive(cw20_msg),
    )
        .unwrap();

    // Query vesting state
    let state =
        from_json::<VestingState>(&query(deps.as_ref(), env, QueryMsg::VestingState {}).unwrap())
            .unwrap();

    assert_eq!(state.total_granted, amount);
    assert_eq!(state.total_released, Uint128::zero());
}

#[test]
fn test_query_timestamp() {
    let (deps, env, _, _) = setup_contract();

    let timestamp =
        from_json::<u64>(&query(deps.as_ref(), env.clone(), QueryMsg::Timestamp {}).unwrap())
            .unwrap();

    assert_eq!(timestamp, env.block.time.seconds());
}

// ========== EXTENSION TESTS ==========

#[test]
fn test_with_managers_extension() {
    let (mut deps, env, owner, _, vesting_managers) = setup_contract_with_extensions();

    // Query vesting managers
    let managers_addrs = from_json::<Vec<Addr>>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::WithManagersExtension {
                msg: QueryMsgWithManagers::VestingManagers {},
            },
        )
            .unwrap(),
    )
        .unwrap();
    let managers_strings: Vec<String> = managers_addrs.iter().map(|a| a.to_string()).collect();
    assert_eq!(managers_strings, vesting_managers);

    // Add vesting managers
    let new_managers = vec![deps.api.addr_make("manager3").into_string()];
    let info = message_info(&owner, &[]);
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::WithManagersExtension {
            msg: ExecuteMsgWithManagers::AddVestingManagers {
                managers: new_managers.clone(),
            },
        },
    )
        .unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "add_vesting_managers");

    // Remove vesting managers
    let info = message_info(&owner, &[]);
    let res = execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::WithManagersExtension {
            msg: ExecuteMsgWithManagers::RemoveVestingManagers {
                managers: vec![vesting_managers[0].clone()],
            },
        },
    )
        .unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "remove_vesting_managers");
}

#[test]
fn test_historical_extension() {
    let (mut deps, env, _, token_info_manager, _) = setup_contract_with_extensions();

    // Set vesting token
    let vesting_token = deps.api.addr_make("vesting_token");
    let info = message_info(&token_info_manager, &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::SetVestingToken {
            vesting_token: token_asset_info(vesting_token),
        },
    )
        .unwrap();

    // Query historical data
    let _result = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::HistoricalExtension {
            msg: QueryMsgHistorical::UnclaimedTotalAmountAtHeight { height: 1000u64 },
        },
    )
        .unwrap();

    let user1 = deps.api.addr_make("user1");
    let _result = query(
        deps.as_ref(),
        env,
        QueryMsg::HistoricalExtension {
            msg: QueryMsgHistorical::UnclaimedAmountAtHeight {
                address: user1.to_string(),
                height: 1000u64,
            },
        },
    )
        .unwrap();
}

#[test]
fn test_managed_extension() {
    let (mut deps, env, owner, token_info_manager, _) = setup_contract_with_extensions();

    // Set vesting token
    let vesting_token = deps.api.addr_make("vesting_token");
    let info = message_info(&token_info_manager, &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::SetVestingToken {
            vesting_token: token_asset_info(vesting_token.clone()),
        },
    )
        .unwrap();

    // Register vesting account first
    let user1 = deps.api.addr_make("user1");
    let amount = Uint128::new(1000);

    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds(),
            amount,
            None,
            None,
        )],
    }];

    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount,
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Receive(cw20_msg),
    )
        .unwrap();

    // Remove vesting accounts
    let clawback_account = deps.api.addr_make("clawback");
    let info = message_info(&owner, &[]);
    let res = execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::ManagedExtension {
            msg: ExecuteMsgManaged::RemoveVestingAccounts {
                vesting_accounts: vec![user1.to_string()],
                clawback_account: clawback_account.to_string(),
            },
        },
    )
        .unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "remove_vesting_accounts");
}

#[test]
fn test_extension_disabled_errors() {
    let (mut deps, env, owner, _) = setup_contract();

    // Test with_managers extension disabled
    let err = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::WithManagersExtension {
            msg: QueryMsgWithManagers::VestingManagers {},
        },
    )
        .unwrap_err();
    assert_eq!(err, ext_unsupported_err("with_managers"));

    // Test historical extension disabled
    let err = query(
        deps.as_ref(),
        env.clone(),
        QueryMsg::HistoricalExtension {
            msg: QueryMsgHistorical::UnclaimedTotalAmountAtHeight { height: 1000u64 },
        },
    )
        .unwrap_err();
    assert_eq!(err, ext_unsupported_err("historical"));

    // Test managed extension disabled
    let info = message_info(&owner, &[]);
    let err = execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::ManagedExtension {
            msg: ExecuteMsgManaged::RemoveVestingAccounts {
                vesting_accounts: vec![],
                clawback_account: String::from("clawback"),
            },
        },
    )
        .unwrap_err();
    assert_eq!(err, ext_unsupported_err("managed").into());
}

// ========== EDGE CASE TESTS ==========

#[test]
fn test_vesting_schedule_edge_cases() {
    let (mut deps, env, owner, _, vesting_token) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");
    let amount = Uint128::new(1000);

    // Test schedule with same start and end time (instant vesting)
    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds(),
            Uint128::new(500),
            Some(env.block.time.seconds()),
            Some(amount),
        )],
    }];

    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount,
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    let err = execute(deps.as_mut(), env, info, ExecuteMsg::Receive(cw20_msg)).unwrap_err();

    assert_eq!(err, ContractError::VestingScheduleError(user1.to_string()));
}

#[test]
fn test_multiple_schedules_same_user() {
    let (mut deps, env, owner, _, vesting_token) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");

    // Register first schedule
    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds(),
            Uint128::new(500),
            None,
            None,
        )],
    }];

    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount: Uint128::new(500),
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Receive(cw20_msg),
    )
        .unwrap();

    // Register second schedule for same user
    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds() + 100,
            Uint128::new(500),
            None,
            None,
        )],
    }];

    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount: Uint128::new(500),
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Receive(cw20_msg),
    )
        .unwrap();

    // Verify user has multiple schedules
    let account = from_json::<VestingAccountResponse>(
        &query(
            deps.as_ref(),
            env,
            QueryMsg::VestingAccount {
                address: user1.to_string(),
            },
        )
            .unwrap(),
    )
        .unwrap();

    assert_eq!(account.info.schedules.len(), 2);
}

#[test]
fn test_zero_amount_claim() {
    let (mut deps, env, owner, _, vesting_token) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");
    let amount = Uint128::new(1000);

    // Register vesting account with future start time
    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds() + 1000, // Future start
            amount,
            None,
            None,
        )],
    }];

    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount,
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Receive(cw20_msg),
    )
        .unwrap();

    // Try to claim before vesting starts (should result in zero claim)
    let info = message_info(&user1, &[]);
    let res = execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::Claim {
            recipient: None,
            amount: None,
        },
    )
        .unwrap();

    // Should have no messages since amount is zero
    assert_eq!(res.messages.len(), 0);
    assert_eq!(res.attributes[3].key, "claimed_amount");
    assert_eq!(res.attributes[3].value, "0");
}

#[test]
fn test_vesting_token_not_set_error() {
    let (mut deps, env, _owner, _) = setup_contract();

    let user1 = deps.api.addr_make("user1");

    // Try to claim without vesting token set
    let info = message_info(&user1, &[]);
    let err = execute(
        deps.as_mut(),
        env,
        info,
        ExecuteMsg::Claim {
            recipient: None,
            amount: None,
        },
    )
        .unwrap_err();

    assert_eq!(err, ContractError::VestingTokenIsNotSet {});
}

#[test]
fn test_register_with_manager() {
    let (mut deps, env, _owner, token_info_manager, _vesting_managers) =
        setup_contract_with_extensions();
    let native_denom = "uatom";

    // Set native token as vesting token (only owner or token_info_manager can do this)
    let info = message_info(&token_info_manager, &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::SetVestingToken {
            vesting_token: native_asset_info(native_denom.to_string()),
        },
    )
        .unwrap();

    let user1 = deps.api.addr_make("user1");
    let amount = Uint128::new(1000);

    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds(),
            amount,
            None,
            None,
        )],
    }];

    // Verify manager is in the list and get the actual manager address
    let managers = from_json::<Vec<Addr>>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::WithManagersExtension {
                msg: QueryMsgWithManagers::VestingManagers {},
            },
        )
            .unwrap(),
    )
        .unwrap();

    // Use the first manager from the actual stored list
    let manager = &managers[0];

    // Manager can register vesting accounts
    let info = message_info(
        manager,
        &[Coin {
            denom: native_denom.to_string(),
            amount,
        }],
    );
    let res = execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::RegisterVestingAccounts { vesting_accounts },
    )
        .unwrap();

    assert_eq!(res.attributes[0].key, "action");
    assert_eq!(res.attributes[0].value, "register_vesting_accounts");
}

#[test]
fn test_full_vesting_lifecycle() {
    let (mut deps, mut env, owner, _, vesting_token) = setup_contract_with_token();

    let user1 = deps.api.addr_make("user1");
    let total_amount = Uint128::new(1000);
    let start_amount = Uint128::new(200);
    let vesting_duration = 100u64;

    // Register vesting account with linear vesting
    let vesting_accounts = vec![VestingAccount {
        address: user1.to_string(),
        schedules: vec![create_vesting_schedule(
            env.block.time.seconds(),
            start_amount,
            Some(env.block.time.seconds() + vesting_duration),
            Some(total_amount),
        )],
    }];

    let info = message_info(&vesting_token, &[]);
    let cw20_msg = Cw20ReceiveMsg {
        sender: owner.to_string(),
        amount: total_amount,
        msg: to_json_binary(&Cw20HookMsg::RegisterVestingAccounts { vesting_accounts }).unwrap(),
    };

    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Receive(cw20_msg),
    )
        .unwrap();

    // Check initial available amount
    let available = from_json::<Uint128>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::AvailableAmount {
                address: user1.to_string(),
            },
        )
            .unwrap(),
    )
        .unwrap();
    assert_eq!(available, start_amount);

    // Claim initial amount
    let info = message_info(&user1, &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Claim {
            recipient: None,
            amount: Some(start_amount),
        },
    )
        .unwrap();

    // Move time forward to middle of vesting
    env.block.time = env.block.time.plus_seconds(vesting_duration / 2);

    let available = from_json::<Uint128>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::AvailableAmount {
                address: user1.to_string(),
            },
        )
            .unwrap(),
    )
        .unwrap();
    // Should be approximately half of the remaining amount vested
    let expected = start_amount + (total_amount - start_amount) / Uint128::new(2);
    assert_eq!(available, expected - start_amount); // Minus already claimed

    // Move to end of vesting
    env.block.time = env.block.time.plus_seconds(vesting_duration / 2);

    let available = from_json::<Uint128>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::AvailableAmount {
                address: user1.to_string(),
            },
        )
            .unwrap(),
    )
        .unwrap();
    assert_eq!(available, total_amount - start_amount); // Full amount minus already claimed

    // Claim remaining amount
    let info = message_info(&user1, &[]);
    execute(
        deps.as_mut(),
        env.clone(),
        info,
        ExecuteMsg::Claim {
            recipient: None,
            amount: None,
        },
    )
        .unwrap();

    // Check final state
    let available = from_json::<Uint128>(
        &query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::AvailableAmount {
                address: user1.to_string(),
            },
        )
            .unwrap(),
    )
        .unwrap();
    assert_eq!(available, Uint128::zero());

    let state =
        from_json::<VestingState>(&query(deps.as_ref(), env, QueryMsg::VestingState {}).unwrap())
            .unwrap();
    assert_eq!(state.total_granted, total_amount);
    assert_eq!(state.total_released, total_amount);
}
