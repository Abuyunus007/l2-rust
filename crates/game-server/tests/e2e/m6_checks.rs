//! Checks that need neither the bare world nor a database.

use super::*;

/// Every action's bytes are a packet the server decodes in game, to the packet the action stands
/// for and without bytes left over. (The bare world never sees the bytes and the real servers run
/// only with a database, so this is what keeps the two halves of an `Act` together.)
#[test]
fn every_act_decodes_on_the_wire() {
    use l2_protocol_game::{ClientPacket, ConnectionState, decode_client};

    let at = m3_data::START;
    let cases: Vec<(Act, &str)> = vec![
        (act::click(1), "Action"),
        (act::move_to(at, at), "MoveToLocation"),
        (act::use_skill(1177), "RequestMagicSkillUse"),
        (act::use_item(1), "UseItem"),
        (act::bypass("npc_1_Buy 1"), "RequestBypassToServer"),
        (act::action_use(22), "RequestActionUse"),
        (act::action_use(10), "RequestActionUse"),
        (act::join_party("Name", 3), "RequestJoinParty"),
        (act::answer_join_party(1), "RequestAnswerJoinParty"),
        (act::withdraw_party(), "RequestWithDrawalParty"),
        (act::trade_request(1), "TradeRequest"),
        (act::answer_trade(1), "AnswerTradeRequest"),
        (act::add_trade_item(1, 2, 3), "AddTradeItem"),
        (act::trade_done(1), "TradeDone"),
        (act::store_msg_sell("Gems"), "SetPrivateStoreMsgSell"),
        (
            act::store_list_sell(false, &[(1, 5, 200)]),
            "SetPrivateStoreListSell",
        ),
        (act::store_buy(1, &[(2, 2, 200)]), "RequestPrivateStoreBuy"),
        (act::buy_items(300101, &[(1835, 5)]), "RequestBuyItem"),
        (act::sell_items(300101, &[(1, 9903, 6)]), "RequestSellItem"),
        (act::shop_ui_close(), "RequestBuySellUIClose"),
        (act::warehouse_deposit(&[(1, 10)]), "SendWareHouseDepositList"),
        (act::warehouse_withdraw(&[(1, 4)]), "SendWareHouseWithDrawList"),
        (act::multisell_choose(400101, 100000, 2), "MultiSellChoose"),
        (act::put_enchant_target(1), "RequestExTryToPutEnchantTargetItem"),
        (act::enchant_item(1), "RequestEnchantItem"),
        (act::join_pledge(1, 0), "RequestJoinPledge"),
        (act::answer_join_pledge(1), "RequestAnswerJoinPledge"),
        (act::set_pledge_crest(&[7; 256]), "RequestSetPledgeCrest"),
        (act::pledge_crest(1), "RequestPledgeCrest"),
        (act::pledge_member_list(), "RequestPledgeMemberList"),
        (
            act::send_post(client::RequestSendPost {
                receiver: "Name".to_owned(),
                is_cod: false,
                subject: "s".to_owned(),
                text: "t".to_owned(),
                items: Some(vec![client::PostAttachment {
                    object_id: 1,
                    count: 7,
                }]),
                req_adena: 0,
            }),
            "RequestSendPost",
        ),
        (act::received_post_list(), "RequestReceivedPostList"),
        (act::received_post(1), "RequestReceivedPost"),
        (act::post_attachment(1), "RequestPostAttachment"),
    ];
    for (act, expected) in cases {
        let mut reader = PacketReader::new(&act.wire[1..]);
        let packet = decode_client(ConnectionState::InGame, act.wire[0], &mut reader)
            .unwrap_or_else(|e| panic!("{expected}: {e:?}"))
            .unwrap_or_else(|| panic!("{expected} is not accepted in game"));
        assert!(
            !matches!(packet, ClientPacket::Unported(_) | ClientPacket::UnportedEx(_)),
            "{expected} is not ported: {packet:?}"
        );
        let debug = format!("{packet:?}");
        assert!(
            debug.starts_with(&format!("{expected}(")) || debug == expected,
            "{expected}: {debug}"
        );
        assert_eq!(reader.remaining(), 0, "{expected} leaves bytes unread");
    }
}

/// What the real-server variants rely on and cannot show without a database: the character the
/// game server creates for the account (the plan it writes to `items` and `character_skills`) owns
/// what the bare players own.
#[test]
fn a_new_fighter_is_created_with_what_the_bare_players_have() {
    use game_server::client::newchar::{
        CreateRequest, IdSource, ItemMaxMp, PlanContext, StatBonus, build_plan,
    };
    use l2_db::ItemLocation;

    let dir = tempfile::tempdir().unwrap();
    let config_dir = dir.path().join("config");
    write_game_config(&config_dir, 4242);
    let data_root = dir.path().join("datapack");
    write_datapack(&data_root);
    let config = GameConfig::load(&config_dir).unwrap();
    assert_eq!(config.player.starting_adena, m6_data::START_ADENA);
    let pack = Datapack::load(&data_root).unwrap();
    let bonus = StatBonus::load(&data_root);
    let max_mp = ItemMaxMp::load(&data_root, &ItemMaxMp::wanted_ids(&pack), false);
    let ctx = PlanContext {
        datapack: &pack,
        config: &config,
        stat_bonus: &bonus,
        item_max_mp: &max_mp,
    };
    let next_id = std::sync::atomic::AtomicI32::new(0x6000_0001);
    let allocate = || Some(next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    let mut ids = IdSource::new(&allocate);
    let request = CreateRequest {
        account: ACCOUNT_A,
        name: NAME_A,
        is_female: false,
        hair_style: 1,
        hair_color: 1,
        face: 1,
    };
    let plan = build_plan(
        &ctx,
        &request,
        pack.player_templates.get_by_id(0).unwrap(),
        0x6000_0000,
        &mut ids,
        &mut WorldRng::from_seed(3),
        0,
    )
    .unwrap();

    let mut made: Vec<(i32, i64, bool)> = plan
        .items
        .iter()
        .map(|i| (i.item_id, i.count, i.loc == ItemLocation::Paperdoll))
        .collect();
    made.sort_unstable();
    let mut wanted: Vec<(i32, i64, bool)> = start_items()
        .into_iter()
        .map(|(item_id, count, slot)| (item_id, count, slot.is_some()))
        .collect();
    wanted.push((ADENA_ID, m6_data::START_ADENA, false));
    wanted.sort_unstable();
    assert_eq!(made, wanted);

    let skills: Vec<i32> = plan.skills.iter().map(|s| s.skill_id).collect();
    for wanted in [
        m5_data::WIND_STRIKE,
        m5_data::WIND_WALK,
        m6_data::CHANT_OF_BATTLE,
        m6_data::SUMMON_KAT,
    ] {
        assert!(skills.contains(&wanted), "{wanted} in {skills:?}");
    }
}

#[test]
fn the_m6_data_loads_as_the_game_server_reads_it() {
    let root = tempfile::tempdir().unwrap();
    write_datapack(root.path());
    let pack = Datapack::load(root.path()).unwrap();

    for (id, kind) in [
        (m6_data::SMITH_ID, "Merchant"),
        (m6_data::KEEPER_ID, "Warehouse"),
        (m6_data::MASTER_ID, "VillageMasterFighter"),
        (m6_data::KAT_ID, "Pet"),
    ] {
        let npc = pack.npcs.get(id).unwrap_or_else(|| panic!("NPC {id}"));
        assert_eq!(npc.npc_type.as_str(), kind);
    }
    for id in [m6_data::CHANT_OF_BATTLE, m6_data::SUMMON_KAT] {
        assert!(pack.skills.get(id, 1).is_some(), "skill {id}");
    }
    for id in [
        m6_data::TEST_BLADE,
        m6_data::ENCHANT_SCROLL_D,
        m6_data::GEM,
        m6_data::ORE,
        m6_data::INGOT,
        m6_data::BLOOD_MARK,
    ] {
        assert!(pack.items.get(id).is_some(), "item {id}");
    }
    assert_eq!(
        i64::from(pack.items.get(m6_data::GEM).unwrap().price),
        m6_data::GEM_PRICE
    );
    let buy_lists = BuyLists::load(&pack.root, &pack.items, &BuyListOptions::default());
    assert!(
        buy_lists
            .get(m6_data::BUY_LIST)
            .is_some_and(|l| l.is_npc_allowed(m6_data::SMITH_ID))
    );
    assert!(pack.multisell.get(m6_data::MULTISELL_LIST).is_some());
    assert!(pack.enchant_items.scroll(m6_data::ENCHANT_SCROLL_D).is_some());
    // The experience table reaches the leader's level.
    assert_eq!(
        pack.experience.exp_for_level(m6_data::LEADER_LEVEL),
        Some(m6_data::LEADER_EXP)
    );
}
