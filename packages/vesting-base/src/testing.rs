use crate::asset::token_asset_info;
use crate::builder::VestingBaseBuilder;
use crate::error::{ext_unsupported_err, ContractError};
use crate::handlers::{execute, query};
use crate::msg::{
    ExecuteMsg, ExecuteMsgManaged, QueryMsg, QueryMsgHistorical, QueryMsgWithManagers,
};
use crate::types::{Config, Extensions};
use cosmwasm_std::testing::{message_info, mock_dependencies, mock_env};
use cosmwasm_std::{from_json, Addr};

#[test]
fn set_vesting_token() {
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

    let info = message_info(&Addr::unchecked("stranger"), &[]);
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
            owner: owner,
            token_info_manager: token_info_manager,
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
fn proper_building_standard() {
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
            owner: owner,
            token_info_manager: token_info_manager,
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
fn proper_building_managers() {
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
            owner: owner,
            token_info_manager: token_info_manager,
            vesting_token: None,
            extensions: Extensions {
                historical: false,
                managed: false,
                with_managers: true
            }
        }
    );

    // make sure with_managers extension is enabled
    assert_eq!(
        from_json::<Vec<String>>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::WithManagersExtension {
                    msg: QueryMsgWithManagers::VestingManagers {},
                },
            )
            .unwrap()
        )
        .unwrap(),
        vesting_managers
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
                    clawback_account: String::from("clawback"),
                },
            },
        )
        .unwrap_err(),
        ext_unsupported_err("managed").into()
    );
}

#[test]
fn proper_building_historical() {
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
            owner: owner,
            token_info_manager: token_info_manager,
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
fn proper_building_managed() {
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
fn proper_building_all_extensions() {
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
    assert_eq!(
        from_json::<Vec<String>>(
            &query(
                deps.as_ref(),
                env.clone(),
                QueryMsg::WithManagersExtension {
                    msg: QueryMsgWithManagers::VestingManagers {},
                },
            )
            .unwrap()
        )
        .unwrap(),
        vesting_managers
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
